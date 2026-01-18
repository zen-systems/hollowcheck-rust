// Test file with hallucinated Go dependencies
package main

import (
	"fmt"    // stdlib - should be ignored
	"os"     // stdlib - should be ignored
	"net/http"  // stdlib - should be ignored

	"github.com/gorilla/mux"  // real package - should pass
	"github.com/spf13/cobra"  // real package - should pass

	// These are fake packages that should be flagged
	"github.com/nonexistent/ai-generated-package-12345"
	"github.com/fake/utils-library-xyz"
	"github.com/totally/made-up-sdk-abc123"
)

func main() {
	fmt.Println("Hello, world!")
}
