{.push overflowChecks: off.}
var n = 1_000_000
var xs: seq[int64] = @[]
var i: int64 = 0
while i < n:
  xs.add(i)
  i = i + 1
var s: int64 = 0
var j = 0
while j < xs.len:
  s = s + xs[j]
  j = j + 1
echo s
