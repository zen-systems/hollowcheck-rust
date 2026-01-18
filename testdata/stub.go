// Package testdata contains test fixtures for hollowcheck.
package testdata

// StubConfig is a placeholder configuration type.
type StubConfig struct {
	Name string
}

// DefaultTimeout is a stub constant.
const DefaultTimeout = 30

// ProcessData is a stub function that does nothing useful.
// TODO: implement actual processing logic
func ProcessData(input string) error {
	// FIXME: this is a placeholder implementation
	return nil
}

// ValidateInput is another stub.
// HACK: bypassing validation for now
func ValidateInput(data []byte) bool {
	// XXX: need to add proper validation
	return true
}

// HandleRequest is a stub handler.
func HandleRequest() error {
	panic("not implemented")
}

// NotImplementedFunc demonstrates a stub pattern.
func NotImplementedFunc() {
	// This function intentionally left empty
	return
}
