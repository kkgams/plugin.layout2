import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

// Exercise the standalone module's pure protocol layer without a DOM/Host mock.
const source = await readFile(new URL('../src/ui-plugins/layout2.js', import.meta.url), 'utf8')
const pure = source.slice(source.indexOf('export class SerialQueue'), source.indexOf('// Browser implementation'))
const { SerialQueue, EngineSession, validateLayout, validateEngineResponse, unwrapLayoutResult, migrateSavedDocument, savedAttributes, dividerPosition, handleBounds } = await import(`data:text/javascript;base64,${Buffer.from(pure).toString('base64')}`)
const registry = new Map([['view-files', { create() {} }], ['view-internal', { internal: true, create() {} }]])
const config = () => ({ state: null, views: {}, defaultView: 'view-files', engine: { gap: 0, min_width: 100, min_height: 80 } })
const response = revision => ({ document: { version: 2, revision, workspaces: [] }, snapshot: { revision, 'active-workspace': 'main', workspaces: [], panels: [], splits: [] } })

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

test('engine session invokes individual typed exports with the current document and positional arguments', async () => {
    const inputs = []
    const session = new EngineSession(async (method, ...args) => {
        inputs.push([method, ...args])
        return response(method === 'initialize' ? 1 : args[0].revision + 1)
    })
    const settings = { gap: 0, 'min-width': 100, 'min-height': 80 }
    await session.run(() => session.initialize(800, 600, settings))
    const first = session.document
    await session.run(() => session.request('focus-direction', ['right'], false))
    assert.equal(session.document.revision, 1)
    await session.run(() => session.request('move-panel', ['a', 'b', 'right', 0.5]))
    assert.equal(session.document.revision, 2)
    assert.deepEqual(inputs[0], ['initialize', 800, 600, settings])
    assert.deepEqual(inputs[1], ['focus-direction', first, 'right'])
    assert.deepEqual(inputs[2], ['move-panel', first, 'a', 'b', 'right', 0.5])
    assert.equal(typeof inputs[2][1], 'object')
})

test('invalid typed responses never overwrite accepted documents', async () => {
    let invalid = false
    const session = new EngineSession(async () => invalid ? {} : response(1))
    await session.initialize(800, 600, {})
    invalid = true
    await assert.rejects(session.request('focus-panel', ['a']), /Layout document/)
    assert.equal(session.document.revision, 1)
    assert.throws(() => validateEngineResponse('{}'), /object/)
})

test('typed layout errors preserve their code and message without JSON parsing', () => {
    assert.deepEqual(unwrapLayoutResult({ ok: { document: {} } }), { document: {} })
    assert.throws(() => unwrapLayoutResult({ err: { code: 'unknown-panel', message: 'Missing a' } }), error => error.code === 'unknown-panel' && error.message === '[unknown-panel] Missing a')
    assert.throws(() => unwrapLayoutResult({ err: 'old opaque error' }), /object/)
    assert.throws(() => unwrapLayoutResult({}), /ok or err/)
})

test('snapshot reads call get-snapshot without changing the caller-owned document', async () => {
    const calls = [], snapshot = response(4).snapshot
    const session = new EngineSession(async (method, document) => { calls.push([method, document]); return snapshot })
    await assert.rejects(session.readSnapshot(), /not initialized/)
    session.document = response(4).document
    const document = session.document
    assert.equal(await session.readSnapshot(), snapshot)
    assert.equal(session.document, document)
    assert.deepEqual(calls, [['get-snapshot', document]])
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

test('saving retains configured/source attributes without transient View markers', () => {
    const configured = { title: 'Files', 'data-source': 'old.lua' }
    assert.deepEqual(savedAttributes(configured, [['title', 'Files'], ['data-source', 'new.lua'], ['data-root', '.'], ['data-ready', '1'], ['style', 'display:contents']]), { title: 'Files', 'data-source': 'new.lua', 'data-root': '.' })
    assert.equal(configured['data-source'], 'old.lua')
    assert.deepEqual(savedAttributes(configured, [['title', 'Files']]), { title: 'Files' })
})

test('typed evaluated layouts stay uncommitted until required Views are ready', async () => {
    let revision = 1
    const session = new EngineSession(async () => response(revision))
    await session.initialize(800, 600, {})
    revision = 2
    const pending = await session.evaluate('create-panel', 'b', null, 'right', 0.5)
    assert.equal(pending.document.revision, 2)
    assert.equal(session.document.revision, 1)
    session.accept(pending)
    assert.equal(session.document.revision, 2)
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

function savedV1() {
    return { version: 1, revision: 7, width: 800, height: 600, gap: 0, min_width: 100, min_height: 80,
        next_split_id: 8, active_workspace: 'main', workspaces: [{ id: 'main', name: 'Main', focused: 'b',
            tree: { kind: 'split', id: 7, vertical: true, ratio: 0.6, first: { kind: 'leaf', id: 'a' }, second: { kind: 'leaf', id: 'b' } } }] }
}

test('saved version-1 layouts migrate to typed flat documents without losing identity or ratios', () => {
    const old = savedV1(), before = structuredClone(old), document = migrateSavedDocument(old)
    assert.equal(document.version, 2)
    assert.equal(document['next-split-id'], 8)
    assert.equal(document['active-workspace'], 'main')
    assert.equal(document['min-width'], 100)
    assert.equal(document.workspaces[0].focused, 'b')
    assert.equal(document.workspaces[0].root, 0)
    assert.deepEqual(document.workspaces[0].nodes, [
        { case: 'split', value: { id: 7, vertical: true, ratio: 0.6, first: 1, second: 2 } },
        { case: 'panel', value: 'a' }, { case: 'panel', value: 'b' },
    ])
    assert.deepEqual(old, before)
    const typed = migrateSavedDocument(document)
    assert.deepEqual(typed, document)
    assert.notEqual(typed, document)
    assert.equal(migrateSavedDocument(null), null)
})

test('saved layout migration rejects unknown versions, malformed trees, cycles and excessive depth', () => {
    assert.throws(() => migrateSavedDocument({ version: 9 }), /Unsupported/)
    const cyclic = savedV1(); cyclic.workspaces[0].tree.first = cyclic.workspaces[0].tree
    assert.throws(() => migrateSavedDocument(cyclic), /Invalid saved BSP tree/)
    const badKind = savedV1(); badKind.workspaces[0].tree.kind = 'broken'
    assert.throws(() => migrateSavedDocument(badKind), /node kind/)
    const missing = savedV1(); delete missing.workspaces[0].focused
    assert.throws(() => migrateSavedDocument(missing), /tree\/focus/)
    const deep = savedV1()
    for (let i = 0; i < 65; i++) deep.workspaces[0].tree = { kind: 'split', id: 20 + i, vertical: true, ratio: 0.5, first: deep.workspaces[0].tree, second: { kind: 'leaf', id: `p${i}` } }
    assert.throws(() => migrateSavedDocument(deep), /Invalid saved BSP tree/)
})
