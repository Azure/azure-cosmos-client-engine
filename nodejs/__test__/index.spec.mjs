import test from 'ava'

import * as cosmoscx from '../index.js'

test('version has expected format', (t) => {
  t.regex(cosmoscx.version(), /^\d+\.\d+\.\d+$/)
})
