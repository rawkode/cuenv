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

//export cue_eval_package
func cue_eval_package(dirPath *C.char, packageName *C.char) *C.char {
	// Add recover to catch any panics
	var result *C.char
	defer func() {
		if r := recover(); r != nil {
			errMsg := map[string]string{"error": fmt.Sprintf("Internal error: %v", r)}
			errBytes, _ := json.Marshal(errMsg)
			// Note: In a real panic scenario, this might not work, but it's worth trying
			result = C.CString(string(errBytes))
		}
	}()

	goDir := C.GoString(dirPath)
	goPkg := C.GoString(packageName)

	// Validate inputs
	if goDir == "" {
		errMsg := map[string]string{"error": "Directory path cannot be empty"}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Get expected package name from environment or use default
	expectedPkg := os.Getenv("CUENV_PACKAGE")
	if expectedPkg == "" {
		expectedPkg = "cuenv" // default package name
	}

	// Only allow loading the configured package
	if goPkg != expectedPkg {
		errMsg := map[string]string{"error": fmt.Sprintf("Only '%s' package is supported, got '%s'. Please ensure your .cue files use 'package %s'", expectedPkg, goPkg, expectedPkg)}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Create a registry for module resolution
	registry, err := modconfig.NewRegistry(&modconfig.Config{
		Env: os.Environ(),
	})
	if err != nil {
		errMsg := map[string]string{"error": "Failed to create registry: " + err.Error()}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Load the CUE package from the directory
	// CUE will automatically search for module root in parent directories
	cfg := &load.Config{
		Dir:      goDir,
		Registry: registry,
		Env:      os.Environ(),
	}

	// Load the package - let CUE handle imports properly
	// Don't specify Package in the config - let CUE determine it from the files
	instances := load.Instances([]string{goPkg}, cfg)
	if len(instances) == 0 {
		errMsg := map[string]string{"error": "No CUE instances found in directory"}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Check for load errors
	inst := instances[0]
	if inst.Err != nil {
		errMsg := map[string]string{"error": inst.Err.Error()}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Build the instance
	ctx := cuecontext.New()
	v := ctx.BuildInstance(inst)

	if v.Err() != nil {
		errMsg := map[string]string{"error": v.Err().Error()}
		errBytes, _ := json.Marshal(errMsg)
		result = C.CString(string(errBytes))
		return result
	}

	// Use the same extraction logic as cue_parse_string
	data := extractCueData(v)

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

// extractCueData extracts the structured data from a CUE value
func extractCueData(v cue.Value) map[string]interface{} {
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

	// Schema: #Cuenv has fields at root level
	// - capabilities?: [string]: #Capability
	// - env?: #Env
	// - hooks?: #Hooks
	// - tasks: [string]: #Tasks

	// Extract environment variables from 'env' field if present
	if envField := v.LookupPath(cue.ParsePath("env")); envField.Exists() {
		// Check for 'environment' sub-field (for multi-environment setups)
		if envSubField := envField.LookupPath(cue.ParsePath("environment")); envSubField.Exists() {
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
				result["environments"] = envs
			}
		}
	}

	// Extract capabilities configuration if present (at root level)
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

		result["commands"] = commands
	}

	// Extract tasks configuration if present (at root level)
	// Tasks can be hierarchical with #TaskGroup or flat #Task
	if tasksField := v.LookupPath(cue.ParsePath("tasks")); tasksField.Exists() {
		tasks := extractTasks(tasksField)
		result["tasks"] = tasks
	}

	// Extract hooks configuration if present (at root level)
	if hooksField := v.LookupPath(cue.ParsePath("hooks")); hooksField.Exists() {
		hooks := make(map[string]interface{})

		// Extract onEnter hook(s) - can be a single hook or an array
		if onEnterField := hooksField.LookupPath(cue.ParsePath("onEnter")); onEnterField.Exists() {
			// Try to decode the entire field as a generic interface to detect its type
			var rawValue interface{}
			if err := onEnterField.Decode(&rawValue); err == nil {
				hooks["onEnter"] = rawValue
			}
		}

		// Extract onExit hook(s) - can be a single hook or an array
		if onExitField := hooksField.LookupPath(cue.ParsePath("onExit")); onExitField.Exists() {
			// Try to decode the entire field as a generic interface to detect its type
			var rawValue interface{}
			if err := onExitField.Decode(&rawValue); err == nil {
				hooks["onExit"] = rawValue
			}
		}

		if len(hooks) > 0 {
			result["hooks"] = hooks
		}
	}

	// Extract variables with capability metadata from 'env' field
	vars := result["variables"].(map[string]interface{})

	if envField := v.LookupPath(cue.ParsePath("env")); envField.Exists() {
		iter, _ := envField.Fields()
		for iter.Next() {
			key := iter.Label()
			val := iter.Value()

			// Skip internal CUE fields, private fields, and special keys
			if strings.HasPrefix(key, "_") || strings.HasPrefix(key, "#") ||
				key == "environment" || key == "capabilities" || key == "hooks" || key == "tasks" {
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
	}

	return result
}

// extractTasks recursively extracts tasks from a hierarchical structure
// Tasks can be either #Task (leaf) or #TaskGroup (containing more tasks)
func extractTasks(tasksField cue.Value) map[string]interface{} {
	tasks := make(map[string]interface{})
	iter, _ := tasksField.Fields()
	for iter.Next() {
		taskName := iter.Label()
		taskValue := iter.Value()
		
		// Skip description field at root level
		if taskName == "description" {
			continue
		}
		
		taskConfig := extractTaskNode(taskValue)
		if taskConfig != nil {
			tasks[taskName] = taskConfig
		}
	}
	return tasks
}

// extractTaskNode extracts a single task node which can be either a task or a group
func extractTaskNode(taskValue cue.Value) map[string]interface{} {
	taskConfig := make(map[string]interface{})
	
	// Check if this is a task with a command field (leaf task)
	if cmdField := taskValue.LookupPath(cue.ParsePath("command")); cmdField.Exists() {
		// It's a #Task
		var cmd string
		if err := cmdField.Decode(&cmd); err == nil {
			taskConfig["command"] = cmd
		}
		
		// Extract shell
		if shellField := taskValue.LookupPath(cue.ParsePath("shell")); shellField.Exists() {
			var shell string
			if err := shellField.Decode(&shell); err == nil {
				taskConfig["shell"] = shell
			}
		}
		
		// Extract args
		if argsField := taskValue.LookupPath(cue.ParsePath("args")); argsField.Exists() {
			var args []string
			if err := argsField.Decode(&args); err == nil {
				taskConfig["args"] = args
			}
		}
		
		// Extract dependencies
		if depsField := taskValue.LookupPath(cue.ParsePath("dependencies")); depsField.Exists() {
			var deps []string
			if err := depsField.Decode(&deps); err == nil {
				taskConfig["dependencies"] = deps
			}
		}
		
		// Extract inputs
		if inputsField := taskValue.LookupPath(cue.ParsePath("inputs")); inputsField.Exists() {
			var inputs []string
			if err := inputsField.Decode(&inputs); err == nil {
				taskConfig["inputs"] = inputs
			}
		}
		
		// Extract outputs
		if outputsField := taskValue.LookupPath(cue.ParsePath("outputs")); outputsField.Exists() {
			var outputs []string
			if err := outputsField.Decode(&outputs); err == nil {
				taskConfig["outputs"] = outputs
			}
		}
		
		return taskConfig
	}
	
	// Check if this has a description field (could be #Tasks wrapper or #TaskGroup)
	if descField := taskValue.LookupPath(cue.ParsePath("description")); descField.Exists() {
		var desc string
		if err := descField.Decode(&desc); err == nil {
			taskConfig["description"] = desc
		}
	}
	
	// It might be a #TaskGroup - check for nested tasks
	iter, _ := taskValue.Fields()
	hasSubTasks := false
	for iter.Next() {
		key := iter.Label()
		if key != "description" && !strings.HasPrefix(key, "_") && !strings.HasPrefix(key, "#") {
			// Found a subtask
			if !hasSubTasks {
				hasSubTasks = true
				taskConfig["tasks"] = make(map[string]interface{})
			}
			subtask := extractTaskNode(iter.Value())
			if subtask != nil {
				taskConfig["tasks"].(map[string]interface{})[key] = subtask
			}
		}
	}
	
	if len(taskConfig) > 0 {
		return taskConfig
	}
	return nil
}

func main() {}
