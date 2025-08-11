package main

/*
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"fmt"
	"os"
	"unsafe"

	"cuelang.org/go/cue/build"
	"cuelang.org/go/cue/cuecontext"
	"cuelang.org/go/cue/load"
)

//export cue_free_string
func cue_free_string(s *C.char) {
	C.free(unsafe.Pointer(s))
}

//export cue_eval_package
func cue_eval_package(dirPath *C.char, packageName *C.char) *C.char {
	// Add recover to catch any panics
	var result *C.char
	defer func() {
		if r := recover(); r != nil {
			errMsg := map[string]string{"error": fmt.Sprintf("Internal error: %v", r)}
			errBytes, _ := json.Marshal(errMsg)
			result = C.CString(string(errBytes))
		}
	}()

	goDir := C.GoString(dirPath)
	// Package name is ignored - we always load from the current directory
	_ = C.GoString(packageName)

	// Validate inputs
	if goDir == "" {
		errMsg := map[string]string{"error": "Directory path cannot be empty"}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Change to the specified directory
	originalDir, err := os.Getwd()
	if err != nil {
		errMsg := map[string]string{"error": fmt.Sprintf("Failed to get current directory: %v", err)}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}
	defer os.Chdir(originalDir) // Always restore original directory

	if err := os.Chdir(goDir); err != nil {
		errMsg := map[string]string{"error": fmt.Sprintf("Failed to change directory to %s: %v", goDir, err)}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Create CUE context
	ctx := cuecontext.New()

	// Load the CUE package from the current directory
	// We always load from "." because the package name is just for validation
	// The actual package directive is in the .cue files
	var instances []*build.Instance
	instances = load.Instances([]string{"."}, nil)

	if len(instances) == 0 {
		errMsg := map[string]string{"error": "No CUE instances found"}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	inst := instances[0]
	if inst.Err != nil {
		errMsg := map[string]string{"error": fmt.Sprintf("Failed to load CUE instance: %v", inst.Err)}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Build the CUE value
	v := ctx.BuildInstance(inst)
	if v.Err() != nil {
		errMsg := map[string]string{"error": fmt.Sprintf("Failed to build CUE value: %v", v.Err())}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Simply decode the entire CUE value as JSON
	var data interface{}
	if err := v.Decode(&data); err != nil {
		errMsg := map[string]string{"error": fmt.Sprintf("Failed to decode CUE value: %v", err)}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Convert to JSON
	jsonBytes, err := json.Marshal(data)
	if err != nil {
		errMsg := map[string]string{"error": err.Error()}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	result = C.CString(string(jsonBytes))
	return result
}

func main() {}