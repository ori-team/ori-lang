n = 1_000_000
xs = []
i = 0
while i < n
  xs.push(i)
  i = i + 1
end
s = 0
i = 0
while i < xs.length
  s = s + xs[i]
  i = i + 1
end
puts s
