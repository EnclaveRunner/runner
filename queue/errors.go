package queue

import (
	"fmt"
)

type InvalidPayloadError struct {
	Msg   string
	Inner error
}

func (e *InvalidPayloadError) Error() string {
	return fmt.Sprintf("Payload invalid: %s: %v", e.Msg, e.Inner)
}

func (e *InvalidPayloadError) Unwrap() error {
	return e.Inner
}

type PullError struct {
	Artifact string
	Err      error
}

func (e *PullError) Error() string {
	return fmt.Sprintf("Failed to pull artifact %q: %v", e.Artifact, e.Err)
}

func (e *PullError) Unwrap() error {
	return e.Err
}

type WasmExecutionError struct {
	Msg   string
	Inner error
}

func (e *WasmExecutionError) Error() string {
	return fmt.Sprintf("WASM execution failed: %s: %v", e.Msg, e.Inner)
}

func (e *WasmExecutionError) Unwrap() error {
	return e.Inner
}
