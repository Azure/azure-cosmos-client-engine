// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build dynamic && windows

package azcosmoscx

// #cgo LDFLAGS: -lcosmoscx
// #include <cosmoscx.h>
import "C"
