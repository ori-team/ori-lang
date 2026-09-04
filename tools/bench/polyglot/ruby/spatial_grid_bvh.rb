def intersect(a, b)
  return false if a[2] < b[0] || a[0] > b[2]
  return false if a[3] < b[1] || a[1] > b[3]
  true
end

target = [10.0, 10.0, 20.0, 20.0]
hits = 0
1000000.times do
  probe = [15.0, 15.0, 25.0, 25.0]
  hits += 1 if intersect(target, probe)
end
puts hits
