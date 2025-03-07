//go:build !azcosmoscx_local && !dynamic && darwin && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/aarch64-apple-darwin/libcosmoscx.a -ldl
// #include <cosmoscx.h>
import "C"
