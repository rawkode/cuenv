package main

/*
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strings"
	"unsafe"

	"cuelang.org/go/cue"
	"cuelang.org/go/cue/cuecontext"
	"cuelang.org/go/cue/load"
	"cuelang.org/go/mod/modconfig"
)

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

	var args []string
	if err := argsField.Decode(&args); err != nil {
		return ""
	}

	resolver := Resolver{Cmd: cmd, Args: args}
	jsonBytes, err := json.Marshal(resolver)
	if err != nil {
		return ""
	}

	// Return with a special prefix so Rust knows this is a secret resolver
	return "secret:" + string(jsonBytes)
}

// escapeJSON escapes a string for safe inclusion in a JSON string
func escapeJSON(s string) string {
	b, _ := json.Marshal(s)
	// Remove the surrounding quotes
	return string(b[1 : len(b)-1])
}

//export cue_free_string
func cue_free_string(s *C.char) {
	C.free(unsafe.Pointer(s))
}

//export cue_eval_package_with_options
func cue_eval_package_with_options(dirPath *C.char, packageName *C.char, environment *C.char, capabilities *C.char) *C.char {
	goDir := C.GoString(dirPath)
	goPkg := C.GoString(packageName)

	var envName string
	if environment != nil {
		envName = C.GoString(environment)
	}

	var capList []string
	if capabilities != nil {
		capsStr := C.GoString(capabilities)
		if capsStr != "" {
			// Parse comma-separated capabilities
			capList = strings.Split(capsStr, ",")
			for i := range capList {
				capList[i] = strings.TrimSpace(capList[i])
			}
		}
	}

	// Only allow loading the "env" package
	if goPkg != "env" {
		return C.CString(`{"error": "only 'env' package is allowed"}`)
	}

	result, err := evalCUEPackageWithOptions(goDir, goPkg, envName, capList)
	if err != nil {
		errMsg := fmt.Sprintf(`{"error": "%s"}`, escapeJSON(err.Error()))
		return C.CString(errMsg)
	}

	jsonBytes, err := json.Marshal(result)
	if err != nil {
		errMsg := fmt.Sprintf(`{"error": "failed to marshal result: %s"}`, escapeJSON(err.Error()))
		return C.CString(errMsg)
	}

	return C.CString(string(jsonBytes))
}

//export cue_eval_package
func cue_eval_package(dirPath *C.char, packageName *C.char) *C.char {
	// Call the new function with nil options for backward compatibility
	return cue_eval_package_with_options(dirPath, packageName, nil, nil)
}

// evalCUEPackageWithOptions evaluates a CUE package with environment and capability options
func evalCUEPackageWithOptions(goDir, goPkg, envName string, capabilities []string) (map[string]interface{}, error) {
	// Create a registry for module resolution
	registry, err := modconfig.NewRegistry(&modconfig.Config{
		Env: os.Environ(),
	})
	if err != nil {
		return nil, fmt.Errorf("Failed to create registry: %w", err)
	}

	// Load the CUE package from the directory
	// CUE will automatically search for module root in parent directories
	cfg := &load.Config{
		Dir:      goDir,
		Package:  goPkg,
		Registry: registry,
		Env:      os.Environ(),
	}

	// Load all .cue files in the directory
	instances := load.Instances(nil, cfg)
	if len(instances) == 0 {
		return nil, fmt.Errorf("No CUE instances found in directory")
	}

	// Check for load errors
	inst := instances[0]
	if inst.Err != nil {
		return nil, inst.Err
	}

	// Build the instance
	ctx := cuecontext.New()
	v := ctx.BuildInstance(inst)

	if v.Err() != nil {
		return nil, v.Err()
	}

	// Use the same extraction logic as extractCueData but with environment merging
	result := extractCueDataWithOptions(v, envName, capabilities)

	return result, nil
}

// extractCueDataWithOptions extracts the structured data from a CUE value with environment and capability filtering
func extractCueDataWithOptions(v cue.Value, envName string, capabilities []string) map[string]interface{} {
	result := map[string]interface{}{
		"variables":    make(map[string]interface{}),
		"metadata":     make(map[string]interface{}),
		"environments": make(map[string]interface{}),
		"commands":     make(map[string]interface{}),
		"tasks":        make(map[string]interface{}),
		"hooks":        nil, // Initialize as nil, will be set if hooks exist
	}

	// Get metadata map reference for use throughout
	metadata := result["metadata"].(map[string]interface{})

	// Look for the 'env' field which contains the environment definition
	envRoot := v.LookupPath(cue.ParsePath("env"))
	if !envRoot.Exists() {
		// Return empty result if no env field
		return result
	}

	// First, extract base variables from env field
	vars := result["variables"].(map[string]interface{})
	iter, _ := envRoot.Fields()
	for iter.Next() {
		key := iter.Label()
		val := iter.Value()

		// Skip internal CUE fields, private fields, and special keys
		if strings.HasPrefix(key, "_") || strings.HasPrefix(key, "#") ||
			key == "environment" || key == "environments" || key == "capabilities" || key == "hooks" || key == "tasks" {
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

	// Extract all environment configurations
	// First check for environments at root level
	envsField := v.LookupPath(cue.ParsePath("environments"))
	if !envsField.Exists() {
		// Check for environment inside env field (legacy location)
		envsField = envRoot.LookupPath(cue.ParsePath("environment"))
	}
	if envsField.Exists() {
		envs := make(map[string]interface{})
		iter, _ := envsField.Fields()
		for iter.Next() {
			envKey := iter.Label()
			envVars := make(map[string]interface{})
			envIter, _ := iter.Value().Fields()
			for envIter.Next() {
				key := envIter.Label()
				val := envIter.Value()

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
			envs[envKey] = envVars
		}
		result["environments"] = envs
	}

	// If an environment is specified, merge environment-specific overrides
	if envName != "" {
		// Try to find the specific environment
		var envField cue.Value

		// First check root level environments
		rootEnvsField := v.LookupPath(cue.ParsePath("environments"))
		if rootEnvsField.Exists() {
			envField = rootEnvsField.LookupPath(cue.ParsePath(envName))
		}

		// If not found, check inside env field
		if !envField.Exists() {
			envEnvsField := envRoot.LookupPath(cue.ParsePath("environment"))
			if envEnvsField.Exists() {
				envField = envEnvsField.LookupPath(cue.ParsePath(envName))
			}
		}

		if envField.Exists() {
			// Merge environment-specific variables
			envIter, _ := envField.Fields()
			for envIter.Next() {
				key := envIter.Label()
				val := envIter.Value()

				// Check if this is a secret type
				secretRef := extractSecretReference(val)
				if secretRef != "" {
					vars[key] = secretRef
				} else {
					// Regular value - override the base value
					var goVal interface{}
					if err := val.Decode(&goVal); err == nil {
						// Convert to string for consistency with env vars
						switch v := goVal.(type) {
						case string:
							vars[key] = v
						case bool:
							vars[key] = fmt.Sprintf("%t", v)
						case int, int32, int64:
							vars[key] = fmt.Sprintf("%d", v)
						case float32, float64:
							vars[key] = fmt.Sprintf("%g", v)
						default:
							vars[key] = fmt.Sprintf("%v", v)
						}
					}
				}
			}
		}
	}

	// Extract capabilities configuration if present
	if capField := v.LookupPath(cue.ParsePath("capabilities")); capField.Exists() {
		caps := make(map[string]interface{})
		iter, _ := capField.Fields()
		for iter.Next() {
			capName := iter.Label()
			capConfig := make(map[string]interface{})
			if cmdsField := iter.Value().LookupPath(cue.ParsePath("commands")); cmdsField.Exists() {
				var cmds []string
				if err := cmdsField.Decode(&cmds); err == nil {
					capConfig["commands"] = cmds
				}
			}
			caps[capName] = capConfig
		}
		result["capabilities"] = caps
	}

	// Build command-to-capabilities mapping from all collected capabilities
	if capsData, ok := result["capabilities"].(map[string]interface{}); ok && len(capsData) > 0 {
		commands := make(map[string]interface{})
		for capName, capConfig := range capsData {
			if capMap, ok := capConfig.(map[string]interface{}); ok {
				if cmds, ok := capMap["commands"].([]string); ok {
					for _, cmd := range cmds {
						if cmdConfig, exists := commands[cmd]; exists {
							// Command already exists, append capability
							if cmdMap, ok := cmdConfig.(map[string]interface{}); ok {
								if caps, ok := cmdMap["capabilities"].([]string); ok {
									cmdMap["capabilities"] = append(caps, capName)
								}
							}
						} else {
							// New command
							commands[cmd] = map[string]interface{}{
								"capabilities": []string{capName},
							}
						}
					}
				}
			}
		}

		// Sort capabilities for each command to ensure deterministic ordering
		for _, cmdConfig := range commands {
			if cmdMap, ok := cmdConfig.(map[string]interface{}); ok {
				if caps, ok := cmdMap["capabilities"].([]string); ok {
					sort.Strings(caps)
					cmdMap["capabilities"] = caps
				}
			}
		}

		// Only include commands in result if there are any
		if len(commands) > 0 {
			result["commands"] = commands
		}
	}

	// Extract tasks if present
	if tasksField := v.LookupPath(cue.ParsePath("tasks")); tasksField.Exists() {
		tasks := make(map[string]interface{})
		iter, _ := tasksField.Fields()
		for iter.Next() {
			taskName := iter.Label()
			taskValue := iter.Value()
			taskData := make(map[string]interface{})

			// Extract all task fields
			fields := []string{"description", "command", "script", "dependencies", "workingDir", "shell", "inputs", "outputs", "cache", "cacheKey", "timeout"}
			for _, field := range fields {
				if fieldValue := taskValue.LookupPath(cue.ParsePath(field)); fieldValue.Exists() {
					var val interface{}
					if err := fieldValue.Decode(&val); err == nil {
						taskData[field] = val
					}
				}
			}

			// Extract security configuration if present
			if secField := taskValue.LookupPath(cue.ParsePath("security")); secField.Exists() {
				security := make(map[string]interface{})
				secFields := []string{"restrictDisk", "restrictNetwork", "readOnlyPaths", "readWritePaths", "denyPaths", "allowedHosts", "inferFromInputsOutputs"}
				for _, field := range secFields {
					if fieldValue := secField.LookupPath(cue.ParsePath(field)); fieldValue.Exists() {
						var val interface{}
						if err := fieldValue.Decode(&val); err == nil {
							security[field] = val
						}
					}
				}
				if len(security) > 0 {
					taskData["security"] = security
				}
			}

			// Extract capabilities for the task
			if capField := taskValue.LookupPath(cue.ParsePath("capabilities")); capField.Exists() {
				var caps []string
				if err := capField.Decode(&caps); err == nil {
					taskData["capabilities"] = caps
				}
			}

			if len(taskData) > 0 {
				tasks[taskName] = taskData
			}
		}
		if len(tasks) > 0 {
			result["tasks"] = tasks
		}
	}

	// Extract hooks configuration if present (at root level, not under env)
	if hooksField := v.LookupPath(cue.ParsePath("hooks")); hooksField.Exists() {
		hooks := make(map[string]interface{})

		// Extract onEnter hook
		if onEnterField := hooksField.LookupPath(cue.ParsePath("onEnter")); onEnterField.Exists() {
			onEnter := make(map[string]interface{})
			if cmdField := onEnterField.LookupPath(cue.ParsePath("command")); cmdField.Exists() {
				var cmd string
				if err := cmdField.Decode(&cmd); err == nil {
					onEnter["command"] = cmd
				}
			}
			if argsField := onEnterField.LookupPath(cue.ParsePath("args")); argsField.Exists() {
				var args []string
				if err := argsField.Decode(&args); err == nil {
					onEnter["args"] = args
				}
			}
			if urlField := onEnterField.LookupPath(cue.ParsePath("url")); urlField.Exists() {
				var url string
				if err := urlField.Decode(&url); err == nil {
					onEnter["url"] = url
				}
			}
			// Extract constraints
			if constraintsField := onEnterField.LookupPath(cue.ParsePath("constraints")); constraintsField.Exists() {
				var constraints []interface{}
				if err := constraintsField.Decode(&constraints); err == nil {
					onEnter["constraints"] = constraints
				}
			}
			if len(onEnter) > 0 {
				hooks["onEnter"] = onEnter
			}
		}

		// Extract onExit hook
		if onExitField := hooksField.LookupPath(cue.ParsePath("onExit")); onExitField.Exists() {
			onExit := make(map[string]interface{})
			if cmdField := onExitField.LookupPath(cue.ParsePath("command")); cmdField.Exists() {
				var cmd string
				if err := cmdField.Decode(&cmd); err == nil {
					onExit["command"] = cmd
				}
			}
			if argsField := onExitField.LookupPath(cue.ParsePath("args")); argsField.Exists() {
				var args []string
				if err := argsField.Decode(&args); err == nil {
					onExit["args"] = args
				}
			}
			if urlField := onExitField.LookupPath(cue.ParsePath("url")); urlField.Exists() {
				var url string
				if err := urlField.Decode(&url); err == nil {
					onExit["url"] = url
				}
			}
			// Extract constraints
			if constraintsField := onExitField.LookupPath(cue.ParsePath("constraints")); constraintsField.Exists() {
				var constraints []interface{}
				if err := constraintsField.Decode(&constraints); err == nil {
					onExit["constraints"] = constraints
				}
			}
			if len(onExit) > 0 {
				hooks["onExit"] = onExit
			}
		}

		if len(hooks) > 0 {
			result["hooks"] = hooks
		}
	}

	// Extract metadata if present
	if metaField := v.LookupPath(cue.ParsePath("metadata")); metaField.Exists() {
		iter, _ := metaField.Fields()
		for iter.Next() {
			varName := iter.Label()
			metaValue := iter.Value()
			varMeta := make(map[string]interface{})

			// Extract capability
			if capField := metaValue.LookupPath(cue.ParsePath("capability")); capField.Exists() {
				var cap string
				if err := capField.Decode(&cap); err == nil {
					varMeta["capability"] = cap
				}
			}

			// Extract sensitive flag
			if sensField := metaValue.LookupPath(cue.ParsePath("sensitive")); sensField.Exists() {
				var sensitive bool
				if err := sensField.Decode(&sensitive); err == nil {
					varMeta["sensitive"] = sensitive
				}
			}

			if len(varMeta) > 0 {
				metadata[varName] = varMeta
			}
		}
	}

	return result
}

func main() {}
