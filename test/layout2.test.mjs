import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

// Exercise the standalone module's pure protocol layer without a DOM/Host mock.
const source = await readFile(new URL('../src/ui-plugins/layout2.js', import.meta.url), 'utf8')
const pure = source.slice(source.indexOf('export class SerialQueue'), source.indexOf('// Browser implementation'))
const { SerialQueue, EngineSession, validateLayout, parseCommand, parseEngineResponse, focusedCommand, savedAttributes, dividerPosition, handleBounds } = await import(`data:text/javascript;base64,${Buffer.from(pure).toString('base64')}`)
const registry = new Map([['view-files', { create() {} }], ['view-internal', { internal: true, create() {} }]])
const config = () => ({ state: null, views: {}, defaultView: 'view-files', engine: { gap: 6, min_width: 100, min_height: 80 } })
const response = revision => JSON.stringify({ state: { revision }, snapshot: { revision, workspaces: [], panels: [], splits: [] } })

test('queue serializes pending commands, resize and save; failed caller rejects without poisoning queue', async () => {
    const queue = new SerialQueue(), order = []
    let release
    const gate = new Promise(resolve => { release = resolve })
    const first = queue.run(async () => { order.push('command'); await gate; order.push('commit') })
    const resize = queue.run(() => { order.push('resize'); throw new Error('bounds') })
    const failed = assert.rejects(resize, /bounds/)
    const save = queue.run(() => order.push('save'))
    await Promise.resolve()
    assert.deepEqual(order, ['command'])
    release()
    await Promise.all([first, failed, save])
    assert.deepEqual(order, ['command', 'commit', 'resize', 'save'])
})

test('engine session serializes exact latest state and supports noncommitting focus probes', async () => {
    const inputs = []
    const session = new EngineSession(async (state, command) => {
        inputs.push([state, JSON.parse(command)])
        return response((state ? JSON.parse(state).revision : 0) + 1)
    })
    await session.run(() => session.request({ op: 'init' }))
    await session.run(() => session.request({ op: 'focus-direction', side: 'right' }, false))
    assert.equal(session.state.revision, 1)
    await session.run(() => session.request({ op: 'move', id: 'a', target: 'b', side: 'right' }))
    assert.equal(session.state.revision, 2)
    assert.deepEqual(inputs.map(([state]) => state), ['', '{"revision":1}', '{"revision":1}'])
})

test('invalid engine responses never overwrite accepted state', async () => {
    let invalid = false
    const session = new EngineSession(async () => invalid ? '{}' : response(1))
    await session.request({ op: 'snapshot' })
    invalid = true
    await assert.rejects(session.request({ op: 'snapshot' }), /Engine state/)
    assert.equal(session.state.revision, 1)
    assert.throws(() => parseEngineResponse({}), /JSON string/)
})

test('layout validation rejects invalid registry entries/settings and preserves independent attributes', () => {
    const input = config()
    input.views['panel-1'] = { tag: 'view-files', attrs: { 'data-source': 'other.lua' } }
    const output = validateLayout(input, registry)
    output.views['panel-1'].attrs['data-source'] = 'changed.lua'
    assert.equal(input.views['panel-1'].attrs['data-source'], 'other.lua')
    for (const modify of [c => { c.engine.gap = -1 }, c => { c.defaultView = 'missing' }, c => { c.defaultView = 'view-internal' }, c => { c.state = [] }, c => { c.views.a = { tag: 'view-files', attrs: { x: 2 } } }]) {
        const invalid = config(); modify(invalid)
        assert.throws(() => validateLayout(invalid, registry))
    }
})

test('commands accept object or JSON using the Rust op discriminator, never legacy type', () => {
    assert.deepEqual(parseCommand('{"op":"snapshot"}'), { op: 'snapshot' })
    const command = { op: 'create', id: 'a' }
    assert.notEqual(parseCommand(command), command)
    assert.throws(() => parseCommand({ type: 'snapshot' }), /Command.op/)
    assert.throws(() => parseCommand('[]'), /object/)
})

test('focused panel commands use the engine op discriminator and cannot override identity', () => {
    assert.deepEqual(focusedCommand('close', 'a'), { op: 'close', id: 'a' })
    assert.deepEqual(focusedCommand('resize', 'a', { side: 'left', delta: 24, op: 'bad', id: 'b' }), { op: 'resize', id: 'a', side: 'left', delta: 24 })
    assert.deepEqual(focusedCommand('workspace-move', 'a', { workspace: 'tools' }), { op: 'workspace-move', id: 'a', workspace: 'tools' })
    assert.throws(() => focusedCommand('close', ''), /No focused panel/)
})

test('saving retains configured/source attributes without transient View markers', () => {
    const configured = { title: 'Files', 'data-source': 'old.lua' }
    assert.deepEqual(savedAttributes(configured, [['title', 'Files'], ['data-source', 'new.lua'], ['data-root', '.'], ['data-ready', '1'], ['style', 'display:contents']]), { title: 'Files', 'data-source': 'new.lua', 'data-root': '.' })
    assert.equal(configured['data-source'], 'old.lua')
    assert.deepEqual(savedAttributes(configured, [['title', 'Files']]), { title: 'Files' })
})

test('evaluated layouts stay uncommitted until required Views are ready', async () => {
    let revision = 1
    const session = new EngineSession(async () => response(revision))
    await session.request({ op: 'snapshot' })
    revision = 2
    const pending = await session.evaluate({ op: 'create', id: 'b' })
    assert.equal(pending.state.revision, 2)
    assert.equal(session.state.revision, 1)
    session.accept(pending)
    assert.equal(session.state.revision, 2)
})

test('special JSON property names remain ordinary saved View input keys', () => {
    const input = config()
    input.views = JSON.parse('{"__proto__":{"tag":"view-files","attrs":{}}}')
    const saved = validateLayout(input, registry)
    assert.equal(Object.hasOwn(saved.views, '__proto__'), true)
    assert.equal(saved.views.__proto__.tag, 'view-files')
    assert.deepEqual(savedAttributes(JSON.parse('{"__proto__":"authored"}'), [['__proto__', 'current']]), JSON.parse('{"__proto__":"current"}'))
})

test('divider pointer positions use viewport coordinates and preserve the grab offset', () => {
    const viewport = { left: 25, top: 80 }
    assert.equal(dividerPosition({ vertical: true }, { clientX: 230.5, clientY: 900 }, viewport, 5.5), 200)
    assert.equal(dividerPosition({ vertical: false }, { clientX: 900, clientY: 284.5 }, viewport, 4.5), 200)
    assert.equal(dividerPosition({ vertical: true }, { clientX: -1e20 }, viewport), -2147483648)
    assert.equal(dividerPosition({ vertical: false }, { clientY: 1e20 }, viewport), 2147483647)
    assert.throws(() => dividerPosition({ vertical: true }, { clientX: NaN }, viewport), /finite/)
})

test('shared handle hit areas are centered on split gaps, including zero-gap layouts', () => {
    assert.deepEqual(handleBounds({ vertical: true, x: 100, y: 20, w: 6, h: 400 }, 12), { x: 97, y: 20, w: 12, h: 400 })
    assert.deepEqual(handleBounds({ vertical: false, x: 30, y: 200, w: 500, h: 6 }, 12), { x: 30, y: 197, w: 500, h: 12 })
    assert.deepEqual(handleBounds({ vertical: true, x: 100, y: 0, w: 0, h: 200 }, 12), { x: 94, y: 0, w: 12, h: 200 })
    assert.deepEqual(handleBounds({ vertical: true, x: 100, y: 0, w: 20, h: 200 }, 12), { x: 100, y: 0, w: 20, h: 200 })
    assert.throws(() => handleBounds({ vertical: true }, 0), /size/)
})
