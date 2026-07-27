{.push overflowChecks: off.}
var n: int64 = 10_000_000
var s: int64 = 0
var i: int64 = 0
while i < n:
  s = s + i
  i = i + 1
echo s
