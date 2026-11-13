import { runtime, unwrap } from "/core/runtime.js"
import { ensureThemeStylesheetLink } from "/util/add-style.js"

export class SerialQueue {
    constructor() { this.tail = Promise.resolve() }
    run(operation) {
        const result = this.tail.then(operation)
        this.tail = result.then(() => undefined, () => undefined)
        return result
    }
}
function object(value, label) {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object`)
    return value
}
export function validateLayout(value, registry) {
    object(value, "layout")
    if (value.state !== null) object(value.state, "layout.state")
    object(value.views, "layout.views"); object(value.engine, "layout.engine")
    for (const field of ["gap", "min_width", "min_height"])
        if (!Number.isInteger(value.engine[field]) || value.engine[field] < (field === "gap" ? 0 : 1) || value.engine[field] > 1_000_000) throw new Error(`Invalid layout.engine.${field}`)
    const check = tag => {
        const entry = registry.get(tag)
        if (!entry || entry.internal === true || typeof entry.create !== "function") throw new Error(`Unknown or internal View '${tag}'`)
    }
    check(value.defaultView)
    for (const [id, spec] of Object.entries(value.views)) {
        if (!id) throw new Error("Panel id is empty")
        object(spec, `View ${id}`); check(spec.tag)
        if (spec.attrs !== undefined) {
            object(spec.attrs, `View ${id} attrs`)
            for (const attr of Object.values(spec.attrs)) if (typeof attr !== "string") throw new Error(`View ${id} attributes must be strings`)
        }
    }
    return structuredClone(value)
}
export function parseCommand(input) {
    const command = object(typeof input === "string" ? JSON.parse(input) : input, "Command")
    if (typeof command.op !== "string" || !command.op) throw new Error("Command.op is required")
    return structuredClone(command)
}
export function focusedCommand(op, id, fields = {}) {
    if (typeof id !== "string" || !id) throw new Error("No focused panel")
    return parseCommand({ ...fields, op, id })
}
export function savedAttributes(configured, attributes) {
    return Object.fromEntries(attributes.filter(([name]) =>
        Object.hasOwn(configured || {}, name) || (name.startsWith("data-") && name !== "data-ready")))
}
export function parseEngineResponse(json) {
    if (typeof json !== "string") throw new Error("Engine must return a JSON string")
    const result = object(JSON.parse(json), "Engine response")
    object(result.state, "Engine state"); object(result.snapshot, "Engine snapshot")
    for (const key of ["workspaces", "panels", "splits"]) if (!Array.isArray(result.snapshot[key])) throw new Error(`Invalid snapshot.${key}`)
    if (!Number.isFinite(result.snapshot.revision)) throw new Error("Snapshot revision is required")
    return result
}
export class EngineSession {
    constructor(invoke) { this.invoke = invoke; this.state = null; this.snapshot = null; this.queue = new SerialQueue() }
    run(operation) { return this.queue.run(operation) }
    async evaluate(command) {
        return parseEngineResponse(await this.invoke(this.state === null ? "" : JSON.stringify(this.state), JSON.stringify(command)))
    }
    accept(result) { this.state = result.state; this.snapshot = result.snapshot }
    async request(command, commit = true) {
        const result = await this.evaluate(command)
        if (commit) this.accept(result)
        return result.snapshot
    }
}

// Browser implementation; the Host loads this file as a blob (no relative imports).
export class ViewArea extends HTMLElement {
    constructor() {
        super()
        this.attachShadow({ mode: "open" }).innerHTML = `<link rel="stylesheet" href="/css/reset.css"><link rel="stylesheet" href="/css/base.css"><style>:host{position:absolute;display:flex;flex-direction:column;overflow:hidden;min-width:0;min-height:0}:host([hidden]){display:none!important}header{flex-shrink:0;display:flex;align-items:center;min-width:0}:host(.active) header{box-shadow:inset 0 0 0 2px var(--accent,Highlight)}main{flex:1;min-width:0;min-height:0;overflow:hidden;display:flex;flex-direction:column}slot:not([name]){display:contents}</style><header part="header"><select part="view-select" class="view-selector" aria-label="Panel View"></select><slot name="header-controls" part="header-controls"></slot></header><main><slot></slot></main>`
        this.shadowRoot.querySelector("select").addEventListener("change", event => { void this.owner.replaceView(this.panel, event.target.value) })
        const focus = () => { if (this.owner.ready) void this.owner.focusPanel(this.panel) }
        this.addEventListener("pointerdown", focus); this.addEventListener("focusin", focus)
    }
    connectedCallback() { ensureThemeStylesheetLink(this.shadowRoot); this.refreshViewSelector() }
    getCurrentView() { return this.firstElementChild }
    getCurrentViewTag() {
        const view = this.getCurrentView()
        if (!view) throw new Error(`Panel '${this.panel}' has no View`)
        return view.tagName.toLowerCase()
    }
    refreshViewSelector() {
        if (!this.owner) throw new Error("view-area owner is required")
        const select = this.shadowRoot.querySelector("select"), groups = new Map()
        select.replaceChildren()
        for (const [tag, entry] of this.owner.viewRegistry) {
            if (entry.internal === true) continue
            const name = entry.group || "Main"
            if (!groups.has(name)) {
                const group = document.createElement("optgroup")
                group.label = name; groups.set(name, group); select.append(group)
            }
            const option = document.createElement("option")
            option.value = tag; option.textContent = entry.label || tag; groups.get(name).append(option)
        }
        select.value = this.getCurrentViewTag()
    }
}
export class UiLayout extends HTMLElement {
    constructor() {
        super()
        this.ready = false; this.viewRegistry = new Map(); this.content = new Map(); this.workspaceNodes = new Map()
        this.session = new EngineSession(async (state, command) => unwrap(await runtime.invoke("layout2/layout::execute", state, command), "Layout execute"))
        this.api = {}
        for (const name of ["execute", "snapshot", "create", "close", "focusDirection", "moveDirection", "move", "resize", "arrange", "equalize", "workspaceCreate", "workspaceSwitch", "workspaceNext", "workspaceRename", "moveToWorkspace", "projectSave", "ping", "refresh"])
            this.api[name] = input => this[name](input)
        this.resizeObserver = new ResizeObserver(() => {
            if (!this.ready) return
            void this.session.run(async () => {
                const bounds = this.bounds(), snapshot = this.session.snapshot
                if (bounds.width !== snapshot.width || bounds.height !== snapshot.height) await this.command({ op: "set-bounds", ...bounds })
            })
        })
    }
    connectedCallback() { this.style.cssText = "position:fixed;inset:0;display:flex;flex-direction:column;overflow:hidden;background:var(--bg)" }
    disconnectedCallback() { this.resizeObserver.disconnect() }
    setViewRegistry(registry) {
        if (!(registry instanceof Map)) throw new Error("View registry must be a Map")
        this.viewRegistry = registry
        for (const chrome of this.content.values()) chrome.refreshViewSelector()
    }
    bounds() { return { width: Math.max(1, Math.floor(this.viewport.clientWidth)), height: Math.max(1, Math.floor(this.viewport.clientHeight)) } }
    load(input) {
        return this.session.run(async () => {
            if (this.ready) throw new Error("Layout already loaded")
            this.settings = validateLayout(input, this.viewRegistry)
            this.settings.views = Object.assign(Object.create(null), this.settings.views)
            this.buildChrome()
            runtime.register({ id: "ui.layout", methods: this.api })
            this.session.state = this.settings.state
            if (this.session.state === null) {
                this.settings.views = Object.assign(Object.create(null), { "panel-1": { tag: this.settings.defaultView } })
                await this.session.request({ op: "init", ...this.bounds(), ...this.settings.engine })
                await this.session.request({ op: "create", id: "panel-1" })
            } else {
                await this.session.request({ op: "snapshot" })
                for (const workspace of this.session.snapshot.workspaces)
                    for (const id of workspace.panels) if (!Object.hasOwn(this.settings.views, id)) throw new Error(`Missing saved View for '${id}'`)
                await this.session.request({ op: "set-bounds", ...this.bounds() })
            }
            await this.render(); this.ready = true; this.resizeObserver.observe(this.viewport)
            return { ok: structuredClone(this.session.snapshot) }
        })
    }
    buildChrome() {
        this.toolbar = document.createElement("nav")
        this.toolbar.setAttribute("aria-label", "Layout actions"); this.toolbar.setAttribute("role", "buttongroup")
        this.toolbar.style.cssText = "display:flex;flex-shrink:0;overflow:auto"
        const button = (text, action) => {
            const node = document.createElement("button")
            node.type = "button"; node.textContent = text
            node.addEventListener("click", () => { void action() }); this.toolbar.append(node)
        }
        button("Split right", () => this.create({ side: "right" }))
        button("Split down", () => this.create({ side: "down" }))
        button("Close panel", () => this.close()); button("Equalize", () => this.equalize())
        for (const kind of ["horizontal", "vertical", "grid"]) button(kind, () => this.arrange({ kind }))
        button("New workspace", () => this.workspaceCreate())
        this.workspaceSelect = document.createElement("select")
        this.workspaceSelect.setAttribute("aria-label", "Move panel to workspace"); this.toolbar.append(this.workspaceSelect)
        button("Move panel", () => this.moveToWorkspace({ workspace: this.workspaceSelect.value }))
        this.renameInput = document.createElement("input"); this.renameInput.type = "text"
        this.renameInput.setAttribute("aria-label", "Workspace name")
        for (const name of ["autocomplete", "autocorrect", "autocapitalize"]) this.renameInput.setAttribute(name, "off")
        this.renameInput.spellcheck = false; this.toolbar.append(this.renameInput)
        button("Rename workspace", () => this.workspaceRename({ name: this.renameInput.value }))
        this.tabs = document.createElement("nav")
        this.tabs.setAttribute("role", "tablist"); this.tabs.setAttribute("aria-label", "Workspaces"); this.tabs.style.flexShrink = "0"
        this.viewport = document.createElement("section")
        this.viewport.style.cssText = "position:relative;flex:1;min-height:0;overflow:hidden"
        this.status = document.createElement("output"); this.status.setAttribute("aria-live", "polite")
        this.append(this.toolbar, this.tabs, this.viewport, this.status)
    }
    requireReady() { if (!this.ready) throw new Error("Layout is not loaded") }
    focused() {
        const id = this.session.snapshot.focused
        if (!id) throw new Error("No focused panel")
        return id
    }
    async createView(spec) {
        const entry = this.viewRegistry.get(spec.tag)
        if (!entry || entry.internal === true) throw new Error(`Unknown or internal View '${spec.tag}'`)
        const attrs = spec.attrs || {}, node = await entry.create({ tag: spec.tag, attrs, innerHTML: "", layout: this })
        for (const [name, value] of Object.entries(attrs)) node.setAttribute(name, value)
        return node
    }
    replaceView(id, tag) {
        return this.session.run(async () => {
            this.requireReady()
            const chrome = this.content.get(id)
            if (!chrome) throw new Error(`Unknown panel '${id}'`)
            let node
            try {
                node = await this.createView({ tag })
            } finally {
                // Restore the current selector even when the new View fails to load.
                chrome.refreshViewSelector()
            }
            // A failed factory leaves the previous View connected.
            chrome.replaceChild(node, chrome.getCurrentView())
            this.settings.views[id] = { tag }; chrome.refreshViewSelector(); await this.activateFocusedView()
            return { ok: true }
        })
    }
    async command(command, additions = new Map()) {
        const result = await this.session.evaluate(command), prepared = []
        for (const workspace of result.snapshot.workspaces) {
            for (const id of workspace.panels) {
                if (this.content.has(id)) continue
                const spec = additions.get(id) || (Object.hasOwn(this.settings.views, id) ? this.settings.views[id] : { tag: this.settings.defaultView })
                prepared.push({ id, spec, node: await this.createView(spec) })
            }
        }
        // Accept only after all required View factories have succeeded.
        this.session.accept(result)
        for (const { id, spec, node } of prepared) { this.settings.views[id] = spec; this.attachPanel(id, node) }
        await this.render()
        return { ok: structuredClone(this.session.snapshot) }
    }
    execute(input) {
        const command = parseCommand(input)
        return this.session.run(async () => { this.requireReady(); return this.command(command) })
    }
    snapshot() { return this.execute({ op: "snapshot" }) }
    ping() { return this.session.run(() => { this.requireReady(); return { ok: { revision: this.session.snapshot.revision } } }) }
    refresh() { return this.session.run(async () => { this.requireReady(); await this.render(); return { ok: structuredClone(this.session.snapshot) } }) }
    create({ side = "right", tag } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            let index = 1
            while (this.content.has(`panel-${index}`)) index++
            const id = `panel-${index}`, spec = { tag: tag === undefined ? this.settings.defaultView : tag }
            return this.command({ op: "create", id, ...(this.session.snapshot.focused ? { target: this.focused(), side } : {}) }, new Map([[id, spec]]))
        })
    }
    close() { return this.focusCommand("close") }
    focusDirection({ side }) { return this.execute({ op: "focus-direction", side }) }
    focusPanel(id) { return this.session.run(async () => this.session.snapshot.focused === id ? { ok: structuredClone(this.session.snapshot) } : this.command({ op: "focus", id })) }
    move(input) { return this.execute({ ...object(input, "move"), op: "move" }) }
    moveDirection({ side }) {
        return this.session.run(async () => {
            this.requireReady()
            const id = this.focused(), probe = await this.session.request({ op: "focus-direction", side }, false)
            if (!probe.focused || probe.focused === id) throw new Error(`No panel ${side} of focused panel`)
            return this.command({ op: "move", id, target: probe.focused, side })
        })
    }
    focusCommand(op, fields = {}) { return this.session.run(async () => { this.requireReady(); return this.command(focusedCommand(op, this.focused(), fields)) }) }
    resize({ side, delta }) { return this.focusCommand("resize", { side, delta }) }
    arrange({ kind }) { return this.execute({ op: "arrange", kind }) }
    equalize() { return this.execute({ op: "equalize" }) }
    workspaceCreate({ name } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            let index = 1
            const ids = new Set(this.session.snapshot.workspaces.map(workspace => workspace.id))
            while (ids.has(`workspace-${index}`)) index++
            return this.command({ op: "workspace-create", id: `workspace-${index}`, name: name === undefined ? `Workspace ${index}` : name })
        })
    }
    workspaceSwitch({ id }) { return this.execute({ op: "workspace-switch", id }) }
    workspaceNext({ delta = 1 } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            if (!Number.isInteger(delta)) throw new Error("Workspace delta must be an integer")
            const snapshot = this.session.snapshot, length = snapshot.workspaces.length
            const index = snapshot.workspaces.findIndex(workspace => workspace.id === snapshot.active_workspace)
            return this.command({ op: "workspace-switch", id: snapshot.workspaces[((index + delta) % length + length) % length].id })
        })
    }
    workspaceRename({ name }) { return this.session.run(async () => { this.requireReady(); return this.command({ op: "workspace-rename", id: this.session.snapshot.active_workspace, name }) }) }
    moveToWorkspace({ workspace }) { return this.focusCommand("workspace-move", { workspace }) }
    attachPanel(id, view) {
        const chrome = document.createElement("view-area")
        chrome.owner = this; chrome.panel = id; chrome.id = `layout2-panel-${encodeURIComponent(id)}`; chrome.setAttribute("panel", id); chrome.tabIndex = -1; chrome.append(view)
        this.content.set(id, chrome)
        // Permanent physical parent: workspace-move must never disconnect live Views.
        this.viewport.append(chrome)
        return chrome
    }
    async render() {
        const snapshot = this.session.snapshot, all = new Set(snapshot.workspaces.flatMap(workspace => workspace.panels))
        const visible = new Set(snapshot.constrained ? [] : snapshot.panels.map(panel => panel.id))
        for (const id of all) {
            if (this.content.has(id)) continue
            const spec = Object.hasOwn(this.settings.views, id) ? this.settings.views[id] : { tag: this.settings.defaultView }
            this.attachPanel(id, await this.createView(spec))
            this.settings.views[id] = spec
        }
        for (const [id, chrome] of this.content) {
            if (all.has(id)) { chrome.hidden = !visible.has(id); continue }
            chrome.remove(); this.content.delete(id); delete this.settings.views[id]
        }
        for (const panel of snapshot.panels) {
            const chrome = this.content.get(panel.id)
            if (!chrome) throw new Error(`Missing panel '${panel.id}'`)
            chrome.hidden = snapshot.constrained
            for (const [field, source] of [["left", "x"], ["top", "y"], ["width", "w"], ["height", "h"]]) chrome.style[field] = `${panel[source]}px`
        }
        for (const [id, chrome] of this.content) chrome.classList.toggle("active", id === snapshot.focused)
        this.renderWorkspaces(snapshot)
        this.status.textContent = snapshot.constrained ? "Layout constrained: enlarge the window to show panels." : ""
        if (!snapshot.constrained && snapshot.focused !== null &&
            (this.lastFocusedPanel !== snapshot.focused || this.lastWorkspace !== snapshot.active_workspace || this.wasConstrained)) {
            this.content.get(snapshot.focused).focus({ preventScroll: true })
        }
        this.lastFocusedPanel = snapshot.focused; this.lastWorkspace = snapshot.active_workspace; this.wasConstrained = snapshot.constrained
        await this.activateFocusedView()
    }
    renderWorkspaces(snapshot) {
        for (const workspace of snapshot.workspaces) {
            let nodes = this.workspaceNodes.get(workspace.id)
            if (!nodes) {
                const tab = document.createElement("button"), panel = document.createElement("section")
                tab.type = "button"; tab.setAttribute("role", "tab"); tab.id = `layout2-tab-${encodeURIComponent(workspace.id)}`
                panel.id = `layout2-workspace-${encodeURIComponent(workspace.id)}`; panel.setAttribute("role", "tabpanel"); panel.setAttribute("aria-labelledby", tab.id); tab.setAttribute("aria-controls", panel.id)
                tab.addEventListener("click", () => { void this.workspaceSwitch({ id: workspace.id }) })
                tab.addEventListener("keydown", event => {
                    const tabs = [...this.tabs.children], index = tabs.indexOf(tab)
                    const next = { ArrowLeft: (index + tabs.length - 1) % tabs.length, ArrowRight: (index + 1) % tabs.length, Home: 0, End: tabs.length - 1 }[event.key]
                    if (next === undefined) return
                    event.preventDefault(); tabs.forEach((item, i) => { item.tabIndex = i === next ? 0 : -1 }); tabs[next].focus()
                })
                this.tabs.append(tab); this.viewport.append(panel); nodes = { tab, panel }; this.workspaceNodes.set(workspace.id, nodes)
            }
            const active = workspace.id === snapshot.active_workspace
            nodes.tab.textContent = workspace.name; nodes.tab.setAttribute("aria-selected", String(active)); nodes.tab.tabIndex = active ? 0 : -1
            nodes.panel.hidden = !active
            // Logical ARIA ownership groups panels without reparenting their DOM.
            nodes.panel.setAttribute("aria-owns", workspace.panels.map(id => `layout2-panel-${encodeURIComponent(id)}`).join(" "))
        }
        const previous = this.workspaceSelect.value
        this.workspaceSelect.replaceChildren()
        for (const workspace of snapshot.workspaces) {
            const option = document.createElement("option")
            option.value = workspace.id; option.textContent = workspace.name; this.workspaceSelect.append(option)
        }
        if (snapshot.workspaces.some(workspace => workspace.id === previous)) this.workspaceSelect.value = previous
    }
    async activateFocusedView() {
        const snapshot = this.session.snapshot
        if (snapshot.focused === null || snapshot.constrained) { unwrap(await runtime.call("ui.context.clearActiveView"), "Clear active View"); return }
        const chrome = this.content.get(snapshot.focused)
        if (!chrome) throw new Error(`Focused panel '${snapshot.focused}' is missing`)
        const view = chrome.getCurrentView()
        if (typeof view.pluginId === "string" && view.pluginId.length > 0) unwrap(await runtime.call("ui.context.activateView", view.pluginId), "Activate View")
        else unwrap(await runtime.call("ui.context.clearActiveView"), "Clear active View")
    }
    projectSave() {
        return this.session.run(async () => {
            this.requireReady()
            const views = Object.create(null)
            for (const [id, chrome] of this.content) {
                const view = chrome.getCurrentView()
                // Preserve configured attributes and live source inputs, not runtime markers.
                const attrs = savedAttributes(this.settings.views[id].attrs, [...view.attributes].map(attr => [attr.name, attr.value]))
                views[id] = { tag: view.tagName.toLowerCase(), attrs }
            }
            const saved = validateLayout({ state: this.session.state, views, defaultView: this.settings.defaultView, engine: this.settings.engine }, this.viewRegistry)
            const service = object(runtime.config.ui.services["ui-layout"], "ui-layout service")
            object(service.config, "ui-layout config").layout = saved
            return { ok: true }
        })
    }
}
customElements.define("view-area", ViewArea)
customElements.define("ui-layout", UiLayout)
