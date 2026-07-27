{.push overflowChecks: off.}
var n: int64 = 20_000_000
var a: int64 = 0
var b: int64 = 1
var i: int64 = 0
while i < n:
  let t = a + b
  a = b
  b = t
  i = i + 1
echo a
