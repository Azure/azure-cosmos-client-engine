// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"

func mapErr(code C.CosmosCxResultCode) error {
	if code == C.COSMOS_CX_RESULT_CODE_SUCCESS {
		return nil
	} else {
		return &Error{code}
	}
}

type Error struct {
	code C.CosmosCxResultCode
}

func (e *Error) Code() uint {
	return uint(e.code)
}

func (e *Error) Error() string {
	switch e.code {
	case C.COSMOS_CX_RESULT_CODE_SUCCESS:
		return "action was successful" // Shouldn't call this, but might as well return something descriptive.
	case C.COSMOS_CX_RESULT_CODE_INVALID_GATEWAY_RESPONSE:
		return "invalid response from gateway"
	case C.COSMOS_CX_RESULT_CODE_DESERIALIZATION_ERROR:
		return "deserialization error"
	case C.COSMOS_CX_RESULT_CODE_UNKNOWN_PARTITION_KEY_RANGE:
		return "unknown partition key range"
	case C.COSMOS_CX_RESULT_CODE_INTERNAL_ERROR:
		return "internal error"
	case C.COSMOS_CX_RESULT_CODE_UNSUPPORTED_QUERY_PLAN:
		return "unsupported query plan"
	case C.COSMOS_CX_RESULT_CODE_INVALID_UTF8_STRING:
		return "invalid UTF-8 string"
	case C.COSMOS_CX_RESULT_CODE_ARGUMENT_NULL:
		return "provided argument was null"
	default:
		return "unknown error"
	}
}
