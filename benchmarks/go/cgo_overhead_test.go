package benchmarks

import (
	"testing"
)

// Benchmark functions for CGO transfer
func BenchmarkCGOTransferNoop(b *testing.B) {
	data := make([]byte, ByteArraySize)
	// Fill with some data
	for i := range data {
		data[i] = byte(i % 256)
	}
	
	b.SetBytes(ByteArraySize)
	b.ResetTimer()
	
	for i := 0; i < b.N; i++ {
		_ = TransferBytesNoop(data)
	}
}

func BenchmarkCGOTransferCopy(b *testing.B) {
	data := make([]byte, ByteArraySize)
	// Fill with some data
	for i := range data {
		data[i] = byte(i % 256)
	}
	
	b.SetBytes(ByteArraySize)
	b.ResetTimer()
	
	for i := 0; i < b.N; i++ {
		_ = TransferBytesCopy(data)
	}
}

func BenchmarkCGOTransferProcess(b *testing.B) {
	data := make([]byte, ByteArraySize)
	// Fill with some data
	for i := range data {
		data[i] = byte(i % 256)
	}
	
	b.SetBytes(ByteArraySize)
	b.ResetTimer()
	
	for i := 0; i < b.N; i++ {
		_ = TransferBytesProcess(data)
	}
}

// Benchmark functions for Go-to-Go transfer
func BenchmarkGoTransferNoop(b *testing.B) {
	data := make([]byte, ByteArraySize)
	// Fill with some data
	for i := range data {
		data[i] = byte(i % 256)
	}
	
	b.SetBytes(ByteArraySize)
	b.ResetTimer()
	
	for i := 0; i < b.N; i++ {
		_ = GoTransferNoop(data)
	}
}

func BenchmarkGoTransferCopy(b *testing.B) {
	data := make([]byte, ByteArraySize)
	// Fill with some data
	for i := range data {
		data[i] = byte(i % 256)
	}
	
	b.SetBytes(ByteArraySize)
	b.ResetTimer()
	
	for i := 0; i < b.N; i++ {
		_ = GoTransferCopy(data)
	}
}

func BenchmarkGoTransferProcess(b *testing.B) {
	data := make([]byte, ByteArraySize)
	// Fill with some data
	for i := range data {
		data[i] = byte(i % 256)
	}
	
	b.SetBytes(ByteArraySize)
	b.ResetTimer()
	
	for i := 0; i < b.N; i++ {
		_ = GoTransferProcess(data)
	}
}
