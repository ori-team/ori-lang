{.push overflowChecks: off.}
var n: int64 = 2000
var s: int64 = 0
var i: int64 = 0
while i < n:
  var j: int64 = 0
  while j < n:
    s = s + 1
    j = j + 1
  i = i + 1
echo s
