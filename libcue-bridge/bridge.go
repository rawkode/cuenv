package main

/*
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"strings"
	"unsafe"

	"cuelang.org/go/cue"
	"cuelang.org/go/cue/cuecontext"
)

//export cue_parse_string
func cue_parse_string(content *C.char) *C.char {
	goContent := C.GoString(content)
	
	// Check if the content starts with "package env" or "package cuenv"
	trimmed := strings.TrimSpace(goContent)
	if !strings.HasPrefix(trimmed, "package env") && !strings.HasPrefix(trimmed, "package cuenv") {
		errMsg := map[string]string{"error": "CUE file must start with 'package env' or 'package cuenv'"}
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
	
	// Extract all fields at the root level with metadata
	result := map[string]interface{}{
		"variables": make(map[string]interface{}),
		"metadata": make(map[string]interface{}),
		"environments": make(map[string]interface{}),
		"commands": make(map[string]interface{}),
	}
	
	// Get metadata map reference for use throughout
	metadata := result["metadata"].(map[string]interface{})
	
	// Extract environment configurations if present
	if envField := v.LookupPath(cue.ParsePath("environment")); envField.Exists() {
		envs := make(map[string]interface{})
		iter, _ := envField.Fields()
		for iter.Next() {
			envName := iter.Label()
			envVars := make(map[string]interface{})
			envMeta := make(map[string]interface{})
			envIter, _ := iter.Value().Fields()
			for envIter.Next() {
				key := envIter.Label()
				val := envIter.Value()
				
				// Extract attributes for environment-specific variables
				attrs := val.Attributes(cue.ValueAttr)
				varMeta := make(map[string]interface{})
				
				for _, attr := range attrs {
					if attr.Name() == "capability" {
						if caps, err := attr.String(0); err == nil {
							varMeta["capability"] = caps
							// Also store in parent metadata if not already there
							if _, exists := metadata[key]; !exists {
								metadata[key] = map[string]interface{}{"capability": caps}
							}
						}
					}
				}
				
				if len(varMeta) > 0 {
					envMeta[key] = varMeta
				}
				
				// Check if this is a secret type
				secretRef := extractSecretReference(val)
				if secretRef != "" {
					envVars[key] = secretRef
				} else {
					// Regular value
					var goVal interface{}
					if err := val.Decode(&goVal); err == nil {
						envVars[key] = goVal
					}
				}
			}
			envs[envName] = envVars
		}
		result["environments"] = envs
	}
	
	// Extract Commands configuration if present
	if cmdField := v.LookupPath(cue.ParsePath("Commands")); cmdField.Exists() {
		cmds := make(map[string]interface{})
		iter, _ := cmdField.Fields()
		for iter.Next() {
			cmdName := iter.Label()
			cmdConfig := make(map[string]interface{})
			if capsField := iter.Value().LookupPath(cue.ParsePath("capabilities")); capsField.Exists() {
				var caps []string
				if err := capsField.Decode(&caps); err == nil {
					cmdConfig["capabilities"] = caps
				}
			}
			cmds[cmdName] = cmdConfig
		}
		result["commands"] = cmds
	}
	
	// Extract variables with capability metadata
	vars := result["variables"].(map[string]interface{})
	
	iter, err := v.Fields()
	if err != nil {
		errMsg := map[string]string{"error": err.Error()}
		errBytes, _ := json.Marshal(errMsg)
		return C.CString(string(errBytes))
	}
	
	for iter.Next() {
		key := iter.Label()
		val := iter.Value()
		
		// Skip internal CUE fields, private fields, and special keys
		if strings.HasPrefix(key, "_") || strings.HasPrefix(key, "#") || 
		   key == "environment" || key == "Commands" {
			continue
		}
		
		// Extract attributes (like @capability)
		attrs := val.Attributes(cue.ValueAttr)
		varMeta := make(map[string]interface{})
		
		for _, attr := range attrs {
			if attr.Name() == "capability" {
				if caps, err := attr.String(0); err == nil {
					varMeta["capability"] = caps
				}
			}
		}
		
		// Check if this is a secret type and convert accordingly
		secretRef := extractSecretReference(val)
		if secretRef != "" {
			vars[key] = secretRef
			if len(varMeta) > 0 {
				metadata[key] = varMeta
			}
		} else {
			// Convert CUE value to Go value
			var goVal interface{}
			if err := val.Decode(&goVal); err == nil {
				vars[key] = goVal
				if len(varMeta) > 0 {
					metadata[key] = varMeta
				}
			}
		}
	}
	
	// Convert to JSON
	jsonBytes, err := json.Marshal(result)
	if err != nil {
		return C.CString("")
	}
	
	return C.CString(string(jsonBytes))
}

// extractSecretReference checks if a CUE value has a resolver field, indicating it's a secret
func extractSecretReference(val cue.Value) string {
	// Check if this value has a resolver field (indicates it's a secret type)
	resolverField := val.LookupPath(cue.ParsePath("resolver"))
	if !resolverField.Exists() {
		return ""
	}
	
	// Extract the resolver configuration
	// Check for both "cmd" and "command" fields for compatibility
	cmdField := resolverField.LookupPath(cue.ParsePath("cmd"))
	if !cmdField.Exists() {
		cmdField = resolverField.LookupPath(cue.ParsePath("command"))
	}
	argsField := resolverField.LookupPath(cue.ParsePath("args"))
	
	if !cmdField.Exists() || !argsField.Exists() {
		return ""
	}
	
	// For now, we'll encode the resolver as a JSON string that the Rust side can decode
	// In the future, this could be a more sophisticated encoding
	type Resolver struct {
		Cmd  string   `json:"cmd"`
		Args []string `json:"args"`
	}
	
	var cmd string
	if err := cmdField.Decode(&cmd); err != nil {
		return ""
	}
	
	// Decode args array
	iter, _ := argsField.List()
	var args []string
	for iter.Next() {
		var arg string
		if err := iter.Value().Decode(&arg); err == nil {
			args = append(args, arg)
		}
	}
	
	resolver := Resolver{
		Cmd:  cmd,
		Args: args,
	}
	
	// Encode as JSON with a special prefix to identify it as a resolver
	jsonBytes, err := json.Marshal(resolver)
	if err != nil {
		return ""
	}
	
	return "cuenv-resolver://" + string(jsonBytes)
}

//export cue_free_string
func cue_free_string(s *C.char) {
	C.free(unsafe.Pointer(s))
}

func main() {}