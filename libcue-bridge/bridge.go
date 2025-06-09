package main

/*
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"strings"
	"unsafe"

	"cuelang.org/go/cue/cuecontext"
)

//export cue_parse_string
func cue_parse_string(content *C.char) *C.char {
	goContent := C.GoString(content)
	
	// Check if the content starts with "package env"
	trimmed := strings.TrimSpace(goContent)
	if !strings.HasPrefix(trimmed, "package env") {
		errMsg := map[string]string{"error": "CUE file must start with 'package env'"}
		errBytes, _ := json.Marshal(errMsg)
		return C.CString(string(errBytes))
	}
	
	ctx := cuecontext.New()
	v := ctx.CompileString(goContent)
	
	if v.Err() != nil {
		// Try to return a more helpful error message
		errMsg := map[string]string{"error": v.Err().Error()}
		errBytes, _ := json.Marshal(errMsg)
		return C.CString(string(errBytes))
	}
	
	// Extract all fields at the root level
	result := make(map[string]interface{})
	iter, err := v.Fields()
	if err != nil {
		errMsg := map[string]string{"error": err.Error()}
		errBytes, _ := json.Marshal(errMsg)
		return C.CString(string(errBytes))
	}
	
	for iter.Next() {
		key := iter.Label()
		val := iter.Value()
		
		// Skip internal CUE fields and private fields
		if strings.HasPrefix(key, "_") || strings.HasPrefix(key, "#") {
			continue
		}
		
		// Convert CUE value to Go value
		var goVal interface{}
		if err := val.Decode(&goVal); err == nil {
			result[key] = goVal
		}
	}
	
	// Convert to JSON
	jsonBytes, err := json.Marshal(result)
	if err != nil {
		return C.CString("")
	}
	
	return C.CString(string(jsonBytes))
}

//export cue_free_string
func cue_free_string(s *C.char) {
	C.free(unsafe.Pointer(s))
}

func main() {}