function fib(n) {
  if (n <= 1) {
    return n
  }

  let a = 0
  let b = 1
  let i = 2
  while (i <= n) {
    const next = a + b
    a = b
    b = next
    i += 1
  }
  return b
}

function fibWork(n, repeatCount) {
  let total = 0
  let i = 0
  while (i < repeatCount) {
    total += fib(n)
    i += 1
  }
  return total
}

function sumSquares(n) {
  let total = 0
  let i = 1
  while (i <= n) {
    total += i * i
    i += 1
  }
  return total
}

function listPushSum(n) {
  const values = []
  let i = 0
  while (i < n) {
    values.push(i * 3 + 7)
    i += 1
  }

  let total = 0
  let j = 0
  while (j < values.length) {
    total += values[j]
    j += 1
  }
  return total
}

const fibAcc = fibWork(32, 80_000)
const sumAcc = sumSquares(200_000)
const listAcc = listPushSum(80_000)
const score = fibAcc + sumAcc + listAcc

console.log(`fib_acc=${fibAcc}`)
console.log(`sum_squares=${sumAcc}`)
console.log(`list_push_sum=${listAcc}`)
console.log(`score=${score}`)
