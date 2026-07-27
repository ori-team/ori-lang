package main

import "fmt"

func main() {
	const n = 1_000_000
	xs := make([]int64, 0, n)
	var i int64 = 0
	for i < int64(n) {
		xs = append(xs, i)
		i++
	}
	var s int64 = 0
	j := 0
	for j < len(xs) {
		s += xs[j]
		j++
	}
	fmt.Println(s)
}
