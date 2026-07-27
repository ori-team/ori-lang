package main

import "fmt"

func main() {
	const n int64 = 20_000_000
	var a int64 = 0
	var b int64 = 1
	var i int64 = 0
	for i < n {
		t := a + b
		a = b
		b = t
		i++
	}
	fmt.Println(a)
}
