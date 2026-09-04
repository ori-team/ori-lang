count = 0
for i in range(100000):
    q = [i]
    v = q.pop(0)
    if v == i:
        count += 1
print(count)
