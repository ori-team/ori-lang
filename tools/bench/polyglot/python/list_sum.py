n = 1_000_000
xs = []
i = 0
while i < n:
    xs.append(i)
    i += 1
s = 0
i = 0
while i < len(xs):
    s += xs[i]
    i += 1
print(s)
