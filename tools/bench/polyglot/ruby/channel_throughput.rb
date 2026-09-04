count = 0
100000.times do |i|
  q = [i]
  v = q.shift
  count += 1 if v == i
end
puts count
