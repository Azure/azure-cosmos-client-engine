package engine

import "unsafe"

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"

func makeStr(s string) C.CosmosCxStr {
	ptr := unsafe.StringData(s)
	return C.CosmosCxStr{
		data: unsafe.Pointer(ptr),
		len:  C.uintptr_t(len(s)),
	}
}

func makeSlice[T any](a []T) C.CosmosCxSlice {
	ptr := unsafe.SliceData(a)
	return C.CosmosCxSlice{
		data: unsafe.Pointer(ptr),
		len:  C.uintptr_t(len(a)),
	}
}
