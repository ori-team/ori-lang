package main

import "fmt"

func main() {
	const n int64 = 10_000_000
	var s int64 = 0
	var i int64 = 0
	for i < n {
		s += i
		i++
	}
	fmt.Println(s)
}
