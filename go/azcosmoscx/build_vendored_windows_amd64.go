//go:build !azcosmoscx_local && !dynamic && windows && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-pc-windows-gnu/libcosmoscx.a -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -ladvapi32 -lntdll -luserenv -lws2_32 -ldbghelp
// #include <cosmoscx.h>
import "C"
