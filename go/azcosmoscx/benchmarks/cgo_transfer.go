package benchmarks

import (
	"unsafe"
)

// #cgo noescape transfer_bytes_noop
// #cgo noescape transfer_bytes_copy
// #cgo noescape transfer_bytes_process
// #cgo nocallback transfer_bytes_noop
// #cgo nocallback transfer_bytes_copy
// #cgo nocallback transfer_bytes_process
/*
#include <stdint.h>
#include <string.h>
#include <stdlib.h>

// Simple C function that just returns the same pointer (no-op)
const uint8_t* transfer_bytes_noop(const uint8_t* data, size_t len) {
    return data;
}

// C function that copies the data to simulate some work
uint8_t* transfer_bytes_copy(const uint8_t* data, size_t len) {
    uint8_t* result = malloc(len);
    memcpy(result, data, len);
    return result;
}

// C function that reads through the data to simulate processing
size_t transfer_bytes_process(const uint8_t* data, size_t len) {
    size_t sum = 0;
    for (size_t i = 0; i < len; i++) {
        sum += data[i];
    }
    return sum;
}
*/
import "C"

// Constants for the benchmark
const (
	ByteArraySize = 1024 // 1KB
)

// CGO wrapper functions
func TransferBytesNoop(data []byte) []byte {
	if len(data) == 0 {
		return data
	}
	ptr := unsafe.Pointer(&data[0])
	length := len(data)
	result := C.transfer_bytes_noop((*C.uint8_t)(ptr), C.size_t(length))
	return C.GoBytes(unsafe.Pointer(result), C.int(length))
}

func TransferBytesCopy(data []byte) []byte {
	if len(data) == 0 {
		return data
	}
	ptr := unsafe.Pointer(&data[0])
	length := len(data)
	result := C.transfer_bytes_copy((*C.uint8_t)(ptr), C.size_t(length))
	defer C.free(unsafe.Pointer(result))
	return C.GoBytes(unsafe.Pointer(result), C.int(length))
}

func TransferBytesProcess(data []byte) int {
	if len(data) == 0 {
		return 0
	}
	ptr := unsafe.Pointer(&data[0])
	length := len(data)
	result := C.transfer_bytes_process((*C.uint8_t)(ptr), C.size_t(length))
	return int(result)
}

// Pure Go equivalent functions
func GoTransferNoop(data []byte) []byte {
	return data
}

func GoTransferCopy(data []byte) []byte {
	result := make([]byte, len(data))
	copy(result, data)
	return result
}

func GoTransferProcess(data []byte) int {
	sum := 0
	for _, b := range data {
		sum += int(b)
	}
	return sum
}
