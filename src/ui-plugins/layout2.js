import { runtime, unwrap } from "/core/runtime.js"
import { ensureThemeStylesheetLink } from "/util/add-style.js"

export class SerialQueue {
    constructor() {
        this.tail = Promise.resolve()
    }
    run(operation) {
        const result = this.tail.then(operation)
        this.tail = result.then(
            () => undefined,
            () => undefined,
        )
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
    object(value.views, "layout.views")
    object(value.engine, "layout.engine")
    for (const field of ["gap", "min_width", "min_height"])
        if (
            !Number.isInteger(value.engine[field]) ||
            value.engine[field] < (field === "gap" ? 0 : 1) ||
            value.engine[field] > 1_000_000
        )
            throw new Error(`Invalid layout.engine.${field}`)
    const check = (tag) => {
        const entry = registry.get(tag)
        if (!entry || entry.internal === true || typeof entry.create !== "function")
            throw new Error(`Unknown or internal View '${tag}'`)
    }
    check(value.defaultView)
    for (const [id, spec] of Object.entries(value.views)) {
        if (!id) throw new Error("Panel id is empty")
        object(spec, `View ${id}`)
        check(spec.tag)
        if (spec.attrs !== undefined) {
            object(spec.attrs, `View ${id} attrs`)
            for (const attr of Object.values(spec.attrs))
                if (typeof attr !== "string") throw new Error(`View ${id} attributes must be strings`)
        }
    }
    return structuredClone(value)
}
export function savedAttributes(configured, attributes) {
    return Object.fromEntries(
        attributes.filter(
            ([name]) => Object.hasOwn(configured || {}, name) || (name.startsWith("data-") && name !== "data-ready"),
        ),
    )
}
export function unwrapLayoutResult(result) {
    object(result, "Layout result")
    if (Object.hasOwn(result, "err")) {
        const failure = object(result.err, "Layout error")
        if (typeof failure.code !== "string" || typeof failure.message !== "string")
            throw new Error("Invalid typed layout error")
        const error = new Error(`[${failure.code}] ${failure.message}`)
        error.code = failure.code
        throw error
    }
    if (!Object.hasOwn(result, "ok")) throw new Error("Layout result must contain ok or err")
    return result.ok
}
export function validateSnapshot(value) {
    object(value, "Layout snapshot")
    for (const key of ["workspaces", "panels", "splits"])
        if (!Array.isArray(value[key])) throw new Error(`Invalid snapshot.${key}`)
    if (!Number.isSafeInteger(value.revision) || typeof value["active-workspace"] !== "string")
        throw new Error("Invalid snapshot revision/workspace")
    return value
}
export function validateEngineResponse(value) {
    object(value, "Layout response")
    object(value.document, "Layout document")
    if (value.document.version !== 2 || !Array.isArray(value.document.workspaces))
        throw new Error("Invalid typed layout document")
    validateSnapshot(value.snapshot)
    return value
}
// Only saved Project configuration needs this one-time conversion. The component
// boundary itself accepts typed version-2 documents, never legacy JSON strings.
export function migrateSavedDocument(value) {
    if (value === null) return null
    object(value, "Saved layout document")
    if (value.version === 2) return structuredClone(value)
    if (value.version !== 1) throw new Error(`Unsupported saved layout version '${value.version}'`)
    if (!Array.isArray(value.workspaces) || value.workspaces.length > 128) throw new Error("Invalid saved workspaces")
    let totalNodes = 0
    const workspaces = value.workspaces.map((workspace) => {
        object(workspace, "Saved workspace")
        const nodes = [],
            visited = new Set()
        const flatten = (node, depth) => {
            object(node, "Saved BSP node")
            if (depth > 64 || visited.has(node) || ++totalNodes > 2047) throw new Error("Invalid saved BSP tree")
            visited.add(node)
            const index = nodes.length
            nodes.push(null)
            if (node.kind === "leaf") nodes[index] = { case: "panel", value: node.id }
            else if (node.kind === "split") {
                nodes[index] = {
                    case: "split",
                    value: {
                        id: node.id,
                        vertical: node.vertical,
                        ratio: node.ratio,
                        first: flatten(node.first, depth + 1),
                        second: flatten(node.second, depth + 1),
                    },
                }
            } else throw new Error(`Invalid saved BSP node kind '${node.kind}'`)
            return index
        }
        if (!Object.hasOwn(workspace, "tree") || !Object.hasOwn(workspace, "focused"))
            throw new Error("Saved workspace tree/focus is required")
        return {
            id: workspace.id,
            name: workspace.name,
            focused: workspace.focused,
            root: workspace.tree === null ? null : flatten(workspace.tree, 1),
            nodes,
        }
    })
    return {
        version: 2,
        revision: value.revision,
        width: value.width,
        height: value.height,
        gap: value.gap,
        "min-width": value.min_width,
        "min-height": value.min_height,
        "next-split-id": value.next_split_id,
        "active-workspace": value.active_workspace,
        workspaces,
    }
}
export class EngineSession {
    constructor(invoke) {
        this.invoke = invoke
        this.document = null
        this.snapshot = null
        this.queue = new SerialQueue()
    }
    run(operation) {
        return this.queue.run(operation)
    }
    requireDocument() {
        if (this.document === null) throw new Error("Layout is not initialized")
    }
    async initialize(width, height, config) {
        const result = validateEngineResponse(await this.invoke("initialize", width, height, config))
        this.accept(result)
        return result.snapshot
    }
    async readSnapshot() {
        this.requireDocument()
        const snapshot = validateSnapshot(await this.invoke("get-snapshot", this.document))
        this.snapshot = snapshot
        return snapshot
    }
    async evaluate(method, ...args) {
        this.requireDocument()
        return validateEngineResponse(await this.invoke(method, this.document, ...args))
    }
    accept(result) {
        this.document = result.document
        this.snapshot = result.snapshot
    }
    async request(method, args = [], commit = true) {
        const result = await this.evaluate(method, ...args)
        if (commit) this.accept(result)
        return result.snapshot
    }
}

export function dividerPosition(split, point, viewport, offset = 0) {
    const position = split.vertical ? point.clientX - viewport.left - offset : point.clientY - viewport.top - offset
    if (!Number.isFinite(position)) throw new Error("Divider position must be finite")
    return Math.max(-2147483648, Math.min(2147483647, Math.round(position)))
}
export function handleBounds(split, thickness) {
    if (!Number.isFinite(thickness) || thickness <= 0) throw new Error("Shared resize handle size must be positive")
    if (split.vertical) {
        const w = Math.max(split.w, thickness)
        return { x: split.x + (split.w - w) / 2, y: split.y, w, h: split.h }
    }
    const h = Math.max(split.h, thickness)
    return { x: split.x, y: split.y + (split.h - h) / 2, w: split.w, h }
}

// Browser implementation; the Host loads this file as a blob (no relative imports).
export class ViewArea extends HTMLElement {
    constructor() {
        super()
        // Preserve the shared Host/theme header and body layout contract.
        this.attachShadow({
            mode: "open",
        }).innerHTML = `<link rel="stylesheet" href="/css/reset.css"><link rel="stylesheet" href="/css/base.css"><style>:host{position:absolute}:host([hidden]){display:none!important}:host(.active) header{box-shadow:inset 0 0 0 2px var(--accent,Highlight)}</style><header part="header"><select part="view-select" class="view-selector" aria-label="Panel View"></select><slot name="header-controls" part="header-controls"></slot></header><main><slot></slot></main>`
        this.shadowRoot.querySelector("select").addEventListener("change", (event) => {
            void this.owner.replaceView(this.panel, event.target.value)
        })
        const focus = () => {
            if (this.owner.ready) void this.owner.focusPanel(this.panel)
        }
        this.addEventListener("pointerdown", focus)
        this.addEventListener("focusin", focus)
    }
    connectedCallback() {
        ensureThemeStylesheetLink(this.shadowRoot)
        this.refreshViewSelector()
    }
    getCurrentView() {
        return this.firstElementChild
    }
    getCurrentViewTag() {
        const view = this.getCurrentView()
        if (!view) throw new Error(`Panel '${this.panel}' has no View`)
        return view.tagName.toLowerCase()
    }
    refreshViewSelector() {
        if (!this.owner) throw new Error("view-area owner is required")
        const select = this.shadowRoot.querySelector("select"),
            groups = new Map()
        select.replaceChildren()
        for (const [tag, entry] of this.owner.viewRegistry) {
            if (entry.internal === true) continue
            const name = entry.group || "Main"
            if (!groups.has(name)) {
                const group = document.createElement("optgroup")
                group.label = name
                groups.set(name, group)
                select.append(group)
            }
            const option = document.createElement("option")
            option.value = tag
            option.textContent = entry.label || tag
            groups.get(name).append(option)
        }
        select.value = this.getCurrentViewTag()
    }
}
export class ResizeHandle extends HTMLElement {
    constructor() {
        super()
        this.gesture = null
        this.addEventListener("pointerdown", (event) => {
            if (this.gesture || !event.isPrimary || (event.pointerType === "mouse" && event.button !== 0)) return
            event.preventDefault()
            this.beginDrag(event)
            this.setPointerCapture(event.pointerId)
        })
        this.addEventListener("pointermove", (event) => this.moveDrag(event))
        this.addEventListener("pointerup", (event) => {
            void this.endDrag(event, false)
        })
        this.addEventListener("pointercancel", (event) => {
            void this.endDrag(event, true)
        })
        this.addEventListener("lostpointercapture", (event) => {
            if (this.gesture && !this.gesture.ending) void this.endDrag(event, true)
        })
        this.addEventListener("keydown", (event) => {
            if (event.key === "Escape" && this.gesture) {
                event.preventDefault()
                void this.endDrag({ pointerId: this.gesture.pointerId }, true)
                return
            }
            if (event.metaKey || event.ctrlKey || event.altKey) return
            const step = event.shiftKey ? 1 : 10
            const deltas = this.split.vertical
                ? { ArrowLeft: -step, ArrowRight: step }
                : { ArrowUp: -step, ArrowDown: step }
            const current = this.split.vertical ? this.split.x : this.split.y
            const position =
                event.key === "Home"
                    ? -2147483648
                    : event.key === "End"
                      ? 2147483647
                      : Object.hasOwn(deltas, event.key)
                        ? current + deltas[event.key]
                        : null
            if (position === null) return
            event.preventDefault()
            this.abortDrag()
            void this.owner.resizeDivider({ splitId: this.split.id, position })
        })
    }
    connectedCallback() {
        this.style.position = "absolute"
        this.style.touchAction = "none"
        this.style.userSelect = "none"
    }
    disconnectedCallback() {
        this.abortDrag()
    }
    beginDrag(event) {
        this.owner.requireReady()
        this.owner.cancelHandleDrags()
        this.focus({ preventScroll: true })
        const bounds = this.owner.viewport.getBoundingClientRect()
        this.gesture = {
            pointerId: event.pointerId,
            split: this.split,
            workspace: this.owner.session.snapshot["active-workspace"],
            offset: this.split.vertical
                ? event.clientX - bounds.left - this.split.x
                : event.clientY - bounds.top - this.split.y,
            position: null,
            frame: 0,
            inFlight: null,
            ending: false,
            cancelled: false,
        }
    }
    position(event, gesture) {
        return dividerPosition(gesture.split, event, this.owner.viewport.getBoundingClientRect(), gesture.offset)
    }
    moveDrag(event) {
        const gesture = this.gesture
        if (!gesture || gesture.ending || event.pointerId !== gesture.pointerId) return
        gesture.position = this.position(event, gesture)
        this.queueFrame(gesture)
    }
    queueFrame(gesture) {
        if (gesture.frame || gesture.inFlight) return
        gesture.frame = requestAnimationFrame(() => {
            gesture.frame = 0
            void this.flushDrag(gesture)
        })
    }
    async flushDrag(gesture) {
        if (this.gesture !== gesture || gesture.cancelled || gesture.position === null) return
        const position = gesture.position
        gesture.position = null
        const current = () =>
            this.gesture === gesture &&
            !gesture.cancelled &&
            this.isConnected &&
            this.owner.session.snapshot["active-workspace"] === gesture.workspace
        const operation = this.owner.session.run(async () => {
            if (!current()) return
            await this.owner.mutate("resize-divider", [gesture.split.id, position], new Map(), current)
        })
        gesture.inFlight = operation
        try {
            await operation
        } finally {
            gesture.inFlight = null
            if (this.gesture === gesture && !gesture.ending && gesture.position !== null) this.queueFrame(gesture)
        }
    }
    async endDrag(event, cancelled) {
        const gesture = this.gesture
        if (!gesture || gesture.ending || event.pointerId !== gesture.pointerId) return
        gesture.ending = true
        gesture.cancelled = cancelled
        if (gesture.frame) cancelAnimationFrame(gesture.frame)
        gesture.frame = 0
        if (this.hasPointerCapture(gesture.pointerId)) this.releasePointerCapture(gesture.pointerId)
        try {
            if (cancelled) {
                gesture.position = null
                await this.owner.session.run(async () => {
                    if (this.gesture !== gesture) return
                    this.gesture = null
                    await this.owner.mutate("set-split-ratio", [gesture.split.id, gesture.split.ratio])
                })
            } else {
                gesture.position = this.position(event, gesture)
                if (gesture.inFlight) await gesture.inFlight
                await this.flushDrag(gesture)
            }
        } finally {
            if (this.gesture === gesture) this.gesture = null
        }
    }
    abortDrag() {
        const gesture = this.gesture
        if (!gesture) return
        this.gesture = null
        if (gesture.frame) cancelAnimationFrame(gesture.frame)
        if (this.hasPointerCapture(gesture.pointerId)) this.releasePointerCapture(gesture.pointerId)
    }
}

export class UiLayout extends HTMLElement {
    constructor() {
        super()
        this.ready = false
        this.viewRegistry = new Map()
        this.content = new Map()
        this.workspaceNodes = new Map()
        this.handles = new Map()
        this.session = new EngineSession(async (method, ...args) =>
            unwrapLayoutResult(await runtime.invoke(`layout2/layout::${method}`, ...args)),
        )
        this.api = {}
        for (const name of [
            "snapshot",
            "create",
            "close",
            "focus",
            "focusDirection",
            "moveDirection",
            "move",
            "moveToRoot",
            "swap",
            "resize",
            "setSplitRatio",
            "resizeDivider",
            "setBounds",
            "arrange",
            "equalize",
            "workspaceCreate",
            "workspaceSwitch",
            "workspaceNext",
            "workspaceRename",
            "workspaceRemove",
            "moveToWorkspace",
            "projectSave",
            "ping",
            "refresh",
        ])
            this.api[name] = (input) => this[name](input)
        this.resizeObserver = new ResizeObserver(() => {
            if (!this.ready) return
            void this.session.run(async () => {
                const bounds = this.bounds(),
                    snapshot = this.session.snapshot
                if (bounds.width !== snapshot.width || bounds.height !== snapshot.height)
                    await this.mutate("set-bounds", [bounds.width, bounds.height])
            })
        })
    }
    connectedCallback() {
        this.style.cssText =
            "position:fixed;inset:0;display:flex;flex-direction:column;overflow:hidden;background:var(--bg)"
    }
    disconnectedCallback() {
        this.cancelHandleDrags()
        this.resizeObserver.disconnect()
    }
    setViewRegistry(registry) {
        if (!(registry instanceof Map)) throw new Error("View registry must be a Map")
        this.viewRegistry = registry
        for (const chrome of this.content.values()) chrome.refreshViewSelector()
    }
    bounds() {
        return {
            width: Math.max(1, Math.floor(this.viewport.clientWidth)),
            height: Math.max(1, Math.floor(this.viewport.clientHeight)),
        }
    }
    load(input) {
        return this.session.run(async () => {
            if (this.ready) throw new Error("Layout already loaded")
            this.settings = validateLayout(input, this.viewRegistry)
            this.settings.views = Object.assign(Object.create(null), this.settings.views)
            this.buildChrome()
            runtime.register({ id: "ui.layout", methods: this.api })
            this.session.document = migrateSavedDocument(this.settings.state)
            const bounds = this.bounds()
            if (this.session.document === null) {
                this.settings.views = Object.assign(Object.create(null), {
                    "panel-1": { tag: this.settings.defaultView },
                })
                await this.session.initialize(bounds.width, bounds.height, {
                    gap: this.settings.engine.gap,
                    "min-width": this.settings.engine.min_width,
                    "min-height": this.settings.engine.min_height,
                })
                await this.session.request("create-panel", ["panel-1", null, "right", 0.5])
            } else {
                await this.session.readSnapshot()
                for (const workspace of this.session.snapshot.workspaces)
                    for (const id of workspace.panels)
                        if (!Object.hasOwn(this.settings.views, id)) throw new Error(`Missing saved View for '${id}'`)
                await this.session.request("set-bounds", [bounds.width, bounds.height])
            }
            await this.render()
            this.ready = true
            this.resizeObserver.observe(this.viewport)
            return { ok: structuredClone(this.session.snapshot) }
        })
    }
    buildChrome() {
        this.toolbar = document.createElement("nav")
        this.toolbar.setAttribute("aria-label", "Layout actions")
        this.toolbar.setAttribute("role", "buttongroup")
        this.toolbar.style.cssText = "display:flex;flex-shrink:0;overflow:auto"
        const button = (label, icon, action) => {
            const node = document.createElement("button"),
                symbol = document.createElement("i")
            node.type = "button"
            node.setAttribute("aria-label", label)
            node.title = label
            symbol.setAttribute("aria-hidden", "true")
            symbol.textContent = icon
            node.append(symbol)
            node.addEventListener("click", () => {
                void action()
            })
            this.toolbar.append(node)
            return node
        }
        button("Split right", "vertical_split", () => this.create({ side: "right" }))
        button("Split down", "horizontal_split", () => this.create({ side: "down" }))
        button("Close panel", "close", () => this.close())
        button("Equalize", "balance", () => this.equalize())
        for (const [kind, label, icon] of [
            ["horizontal", "Arrange horizontally", "view_column"],
            ["vertical", "Arrange vertically", "table_rows"],
            ["grid", "Arrange as grid", "grid_view"],
        ])
            button(label, icon, () => this.arrange({ kind }))
        button("New workspace", "add_box", () => this.workspaceCreate())
        this.removeWorkspaceButton = button("Close workspace", "tab_close", () => this.workspaceRemove())
        this.workspaceSelect = document.createElement("select")
        this.workspaceSelect.setAttribute("aria-label", "Move panel to workspace")
        this.toolbar.append(this.workspaceSelect)
        button("Move panel", "drive_file_move", () => this.moveToWorkspace({ workspace: this.workspaceSelect.value }))
        this.renameInput = document.createElement("input")
        this.renameInput.type = "text"
        this.renameInput.setAttribute("aria-label", "Workspace name")
        for (const name of ["autocomplete", "autocorrect", "autocapitalize"]) this.renameInput.setAttribute(name, "off")
        this.renameInput.spellcheck = false
        this.toolbar.append(this.renameInput)
        button("Rename workspace", "edit", () => this.workspaceRename({ name: this.renameInput.value }))
        this.tabs = document.createElement("nav")
        this.tabs.setAttribute("role", "tablist")
        this.tabs.setAttribute("aria-label", "Workspaces")
        this.tabs.style.flexShrink = "0"
        this.viewport = document.createElement("section")
        this.viewport.style.cssText = "position:relative;flex:1;min-height:0;overflow:hidden"
        this.status = document.createElement("output")
        this.status.setAttribute("aria-live", "polite")
        this.append(this.toolbar, this.tabs, this.viewport, this.status)
    }
    requireReady() {
        if (!this.ready) throw new Error("Layout is not loaded")
    }
    focused() {
        const id = this.session.snapshot.focused
        if (!id) throw new Error("No focused panel")
        return id
    }
    async createView(spec) {
        const entry = this.viewRegistry.get(spec.tag)
        if (!entry || entry.internal === true) throw new Error(`Unknown or internal View '${spec.tag}'`)
        const attrs = spec.attrs || {},
            node = await entry.create({ tag: spec.tag, attrs, innerHTML: "", layout: this })
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
            this.settings.views[id] = { tag }
            chrome.refreshViewSelector()
            await this.activateFocusedView()
            return { ok: true }
        })
    }
    async mutate(method, args = [], additions = new Map(), isCurrent = () => true) {
        if (method !== "resize-divider") this.cancelHandleDrags()
        const result = await this.session.evaluate(method, ...args),
            prepared = []
        for (const workspace of result.snapshot.workspaces) {
            for (const id of workspace.panels) {
                if (this.content.has(id)) continue
                const spec =
                    additions.get(id) ||
                    (Object.hasOwn(this.settings.views, id)
                        ? this.settings.views[id]
                        : { tag: this.settings.defaultView })
                prepared.push({ id, spec, node: await this.createView(spec) })
            }
        }
        if (!isCurrent()) return { ok: structuredClone(this.session.snapshot) }
        this.session.accept(result)
        for (const { id, spec, node } of prepared) {
            this.settings.views[id] = spec
            this.attachPanel(id, node)
        }
        await this.render()
        return { ok: structuredClone(this.session.snapshot) }
    }
    operation(method, ...args) {
        return this.session.run(async () => {
            this.requireReady()
            return this.mutate(method, args)
        })
    }
    snapshot() {
        return this.session.run(async () => {
            this.requireReady()
            return { ok: structuredClone(await this.session.readSnapshot()) }
        })
    }
    ping() {
        return this.session.run(() => {
            this.requireReady()
            return { ok: { revision: this.session.snapshot.revision } }
        })
    }
    refresh() {
        return this.session.run(async () => {
            this.requireReady()
            await this.render()
            return { ok: structuredClone(this.session.snapshot) }
        })
    }
    create({ side = "right", tag, share = 0.5 } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            let index = 1
            while (this.content.has(`panel-${index}`)) index++
            const id = `panel-${index}`,
                spec = { tag: tag === undefined ? this.settings.defaultView : tag }
            return this.mutate("create-panel", [id, this.session.snapshot.focused, side, share], new Map([[id, spec]]))
        })
    }
    focusedOperation(method, ...args) {
        return this.session.run(async () => {
            this.requireReady()
            return this.mutate(method, [this.focused(), ...args])
        })
    }
    close() {
        return this.focusedOperation("close-panel")
    }
    focus({ id }) {
        return this.operation("focus-panel", id)
    }
    focusDirection({ side }) {
        return this.operation("focus-direction", side)
    }
    focusPanel(id) {
        return this.session.run(async () => {
            this.requireReady()
            return this.session.snapshot.focused === id
                ? { ok: structuredClone(this.session.snapshot) }
                : this.mutate("focus-panel", [id])
        })
    }
    move({ id, target, side, share = 0.5 }) {
        return this.operation("move-panel", id, target, side, share)
    }
    moveToRoot({ id, side, share = 0.5 }) {
        return this.operation("move-panel-to-root", id, side, share)
    }
    swap({ first, second }) {
        return this.operation("swap-panels", first, second)
    }
    moveDirection({ side }) {
        return this.session.run(async () => {
            this.requireReady()
            const id = this.focused(),
                probe = await this.session.request("focus-direction", [side], false)
            if (!probe.focused || probe.focused === id) throw new Error(`No panel ${side} of focused panel`)
            return this.mutate("move-panel", [id, probe.focused, side, 0.5])
        })
    }
    resize({ side, delta }) {
        return this.focusedOperation("resize-panel", side, delta)
    }
    setSplitRatio({ splitId, ratio }) {
        return this.operation("set-split-ratio", splitId, ratio)
    }
    resizeDivider({ splitId, position }) {
        return this.operation("resize-divider", splitId, position)
    }
    setBounds({ width, height }) {
        return this.operation("set-bounds", width, height)
    }
    arrange({ kind }) {
        return this.operation("arrange", kind)
    }
    equalize() {
        return this.operation("equalize")
    }
    workspaceCreate({ name } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            let index = 1
            const ids = new Set(this.session.snapshot.workspaces.map((workspace) => workspace.id))
            while (ids.has(`workspace-${index}`)) index++
            return this.mutate("create-workspace", [
                `workspace-${index}`,
                name === undefined ? `Workspace ${index}` : name,
            ])
        })
    }
    workspaceSwitch({ id }) {
        return this.operation("switch-workspace", id)
    }
    workspaceRemove({ id } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            return this.mutate("remove-workspace", [id === undefined ? this.session.snapshot["active-workspace"] : id])
        })
    }
    workspaceNext({ delta = 1 } = {}) {
        return this.session.run(async () => {
            this.requireReady()
            if (!Number.isInteger(delta)) throw new Error("Workspace delta must be an integer")
            const snapshot = this.session.snapshot,
                length = snapshot.workspaces.length
            const index = snapshot.workspaces.findIndex((workspace) => workspace.id === snapshot["active-workspace"])
            return this.mutate("switch-workspace", [
                snapshot.workspaces[(((index + delta) % length) + length) % length].id,
            ])
        })
    }
    workspaceRename({ name }) {
        return this.session.run(async () => {
            this.requireReady()
            return this.mutate("rename-workspace", [this.session.snapshot["active-workspace"], name])
        })
    }
    moveToWorkspace({ workspace }) {
        return this.focusedOperation("move-panel-to-workspace", workspace)
    }
    attachPanel(id, view) {
        const chrome = document.createElement("view-area")
        chrome.owner = this
        chrome.panel = id
        chrome.id = `layout2-panel-${encodeURIComponent(id)}`
        chrome.setAttribute("panel", id)
        chrome.tabIndex = -1
        chrome.append(view)
        this.content.set(id, chrome)
        // Permanent physical parent: workspace-move must never disconnect live Views.
        this.viewport.append(chrome)
        return chrome
    }
    async render() {
        const snapshot = this.session.snapshot,
            all = new Set(snapshot.workspaces.flatMap((workspace) => workspace.panels))
        const visible = new Set(snapshot.constrained ? [] : snapshot.panels.map((panel) => panel.id))
        for (const id of all) {
            if (this.content.has(id)) continue
            const spec = Object.hasOwn(this.settings.views, id)
                ? this.settings.views[id]
                : { tag: this.settings.defaultView }
            this.attachPanel(id, await this.createView(spec))
            this.settings.views[id] = spec
        }
        for (const [id, chrome] of this.content) {
            if (all.has(id)) {
                chrome.hidden = !visible.has(id)
                continue
            }
            chrome.remove()
            this.content.delete(id)
            delete this.settings.views[id]
        }
        for (const panel of snapshot.panels) {
            const chrome = this.content.get(panel.id)
            if (!chrome) throw new Error(`Missing panel '${panel.id}'`)
            chrome.hidden = snapshot.constrained
            for (const [field, source] of [
                ["left", "x"],
                ["top", "y"],
                ["width", "w"],
                ["height", "h"],
            ])
                chrome.style[field] = `${panel[source]}px`
        }
        for (const [id, chrome] of this.content) chrome.classList.toggle("active", id === snapshot.focused)
        this.renderHandles(snapshot)
        this.renderWorkspaces(snapshot)
        this.status.textContent = snapshot.constrained ? "Layout constrained: enlarge the window to show panels." : ""
        if (
            !snapshot.constrained &&
            snapshot.focused !== null &&
            (this.lastFocusedPanel !== snapshot.focused ||
                this.lastWorkspace !== snapshot["active-workspace"] ||
                this.wasConstrained)
        ) {
            this.content.get(snapshot.focused).focus({ preventScroll: true })
        }
        this.lastFocusedPanel = snapshot.focused
        this.lastWorkspace = snapshot["active-workspace"]
        this.wasConstrained = snapshot.constrained
        await this.activateFocusedView()
    }
    cancelHandleDrags() {
        for (const handle of this.handles.values()) handle.abortDrag()
    }
    renderHandles(snapshot) {
        const active = new Set(snapshot.splits.map((split) => split.id))
        for (const [id, node] of this.handles) {
            if (active.has(id)) continue
            node.remove()
            this.handles.delete(id)
        }
        for (const split of snapshot.splits) {
            let node = this.handles.get(split.id)
            if (!node) {
                node = document.createElement("view--handle")
                node.owner = this
                node.tabIndex = 0
                node.setAttribute("role", "separator")
                node.setAttribute("aria-label", "Resize panels")
                node.setAttribute("aria-valuemin", "0")
                node.setAttribute("aria-valuemax", "100")
                this.handles.set(split.id, node)
                this.viewport.append(node)
            }
            node.split = split
            node.panel = String(split.id)
            node.setAttribute("panel", node.panel)
            node.setAttribute("aria-orientation", split.vertical ? "vertical" : "horizontal")
            node.setAttribute("aria-valuenow", String(Math.round(split.ratio * 100)))
            // Resolve the shared CSS size token, including rem/calc values,
            // then center that hit area over the BSP gap in either direction.
            node.style.width = "var(--resize-handle-size)"
            const thickness = Number.parseFloat(getComputedStyle(node).width)
            const rect = handleBounds(split, thickness)
            for (const [property, source] of [
                ["left", "x"],
                ["top", "y"],
                ["width", "w"],
                ["height", "h"],
            ]) {
                node.setAttribute(source, String(rect[source]))
                node.style[property] = `${rect[source]}px`
            }
        }
    }
    renderWorkspaces(snapshot) {
        const ids = new Set(snapshot.workspaces.map((workspace) => workspace.id))
        for (const [id, nodes] of this.workspaceNodes) {
            if (ids.has(id)) continue
            nodes.tab.remove()
            nodes.panel.remove()
            this.workspaceNodes.delete(id)
        }
        this.removeWorkspaceButton.disabled = snapshot.workspaces.length === 1
        for (const workspace of snapshot.workspaces) {
            let nodes = this.workspaceNodes.get(workspace.id)
            if (!nodes) {
                const tab = document.createElement("button"),
                    panel = document.createElement("section")
                tab.type = "button"
                tab.setAttribute("role", "tab")
                tab.id = `layout2-tab-${encodeURIComponent(workspace.id)}`
                panel.id = `layout2-workspace-${encodeURIComponent(workspace.id)}`
                panel.setAttribute("role", "tabpanel")
                panel.setAttribute("aria-labelledby", tab.id)
                tab.setAttribute("aria-controls", panel.id)
                tab.addEventListener("click", () => {
                    void this.workspaceSwitch({ id: workspace.id })
                })
                tab.addEventListener("keydown", (event) => {
                    const tabs = [...this.tabs.children],
                        index = tabs.indexOf(tab)
                    const next = {
                        ArrowLeft: (index + tabs.length - 1) % tabs.length,
                        ArrowRight: (index + 1) % tabs.length,
                        Home: 0,
                        End: tabs.length - 1,
                    }[event.key]
                    if (next === undefined) return
                    event.preventDefault()
                    tabs.forEach((item, i) => {
                        item.tabIndex = i === next ? 0 : -1
                    })
                    tabs[next].focus()
                })
                this.tabs.append(tab)
                this.viewport.append(panel)
                nodes = { tab, panel }
                this.workspaceNodes.set(workspace.id, nodes)
            }
            const active = workspace.id === snapshot["active-workspace"]
            nodes.tab.textContent = workspace.name
            nodes.tab.setAttribute("aria-selected", String(active))
            nodes.tab.tabIndex = active ? 0 : -1
            nodes.panel.hidden = !active
            // Logical ARIA ownership groups panels without reparenting their DOM.
            nodes.panel.setAttribute(
                "aria-owns",
                workspace.panels.map((id) => `layout2-panel-${encodeURIComponent(id)}`).join(" "),
            )
        }
        const previous = this.workspaceSelect.value
        this.workspaceSelect.replaceChildren()
        for (const workspace of snapshot.workspaces) {
            const option = document.createElement("option")
            option.value = workspace.id
            option.textContent = workspace.name
            this.workspaceSelect.append(option)
        }
        if (snapshot.workspaces.some((workspace) => workspace.id === previous)) this.workspaceSelect.value = previous
    }
    async activateFocusedView() {
        const snapshot = this.session.snapshot
        if (snapshot.focused === null || snapshot.constrained) {
            unwrap(await runtime.call("ui.context.clearActiveView"), "Clear active View")
            return
        }
        const chrome = this.content.get(snapshot.focused)
        if (!chrome) throw new Error(`Focused panel '${snapshot.focused}' is missing`)
        const view = chrome.getCurrentView()
        if (typeof view.pluginId === "string" && view.pluginId.length > 0)
            unwrap(await runtime.call("ui.context.activateView", view.pluginId), "Activate View")
        else unwrap(await runtime.call("ui.context.clearActiveView"), "Clear active View")
    }
    projectSave() {
        return this.session.run(async () => {
            this.requireReady()
            const views = Object.create(null)
            for (const [id, chrome] of this.content) {
                const view = chrome.getCurrentView()
                // Preserve configured attributes and live source inputs, not runtime markers.
                const attrs = savedAttributes(
                    this.settings.views[id].attrs,
                    [...view.attributes].map((attr) => [attr.name, attr.value]),
                )
                views[id] = { tag: view.tagName.toLowerCase(), attrs }
            }
            const saved = validateLayout(
                {
                    state: this.session.document,
                    views,
                    defaultView: this.settings.defaultView,
                    engine: this.settings.engine,
                },
                this.viewRegistry,
            )
            const service = object(runtime.config.ui.services["ui-layout"], "ui-layout service")
            object(service.config, "ui-layout config").layout = saved
            return { ok: true }
        })
    }
}
customElements.define("view-area", ViewArea)
customElements.define("view--handle", ResizeHandle)
customElements.define("ui-layout", UiLayout)
