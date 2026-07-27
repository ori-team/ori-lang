package main

import "fmt"

func main() {
	const n int64 = 2000
	var s int64 = 0
	var i int64 = 0
	for i < n {
		var j int64 = 0
		for j < n {
			s++
			j++
		}
		i++
	}
	fmt.Println(s)
}
