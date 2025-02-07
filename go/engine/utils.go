package engine

import "unsafe"

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"

func makeStr(s string) C.CosmosCxStr {
	ptr := unsafe.StringData(s)
	return C.CosmosCxStr{
		data: (*C.uint8_t)(ptr),
		len:  C.uintptr_t(len(s)),
	}
}

type BorrowedSlice[T any] struct {
	ptr unsafe.Pointer
	len uint
}

func NewBorrowedSlice[T any](ptr unsafe.Pointer, len uint) BorrowedSlice[T] {
	return BorrowedSlice[T]{ptr, len}
}
