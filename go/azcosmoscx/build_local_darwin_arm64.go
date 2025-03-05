//go:build local && !dynamic && darwin && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-apple-darwin/debug/lib/libcosmoscx.a -ldl
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-apple-darwin/release/lib/libcosmoscx.a -ldl
// #include <cosmoscx.h>
import "C"
