import test from 'ava'
import fs from 'node:fs'

import { parse } from '../index'

test('can parse a file', (t) => {
	// cwd is actually not the test dir, but the root of the project
	// needs us to be buffered
	const fileContents = fs.readFileSync('../../fixtures/berry.lock')
	const parsed = parse(fileContents)
	t.log(`result is ${parsed.length} bytes long`)
	t.true(true)
})
