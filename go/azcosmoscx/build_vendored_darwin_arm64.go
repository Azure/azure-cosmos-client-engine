//go:build !azcosmoscx_local && !dynamic && darwin && arm64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/aarch64-apple-darwin/libcosmoscx.a -lSystem -lc -lm
// #include <cosmoscx.h>
import "C"
