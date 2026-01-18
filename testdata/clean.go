// Package testdata contains test fixtures for hollowcheck.
package testdata

import (
	"errors"
	"strings"
)

// Config represents application configuration.
type Config struct {
	MaxRetries int
	Timeout    int
	Debug      bool
}

// MaxConnections is the maximum number of allowed connections.
const MaxConnections = 100

// ErrInvalidInput is returned when input validation fails.
var ErrInvalidInput = errors.New("invalid input")

// Validate checks if the configuration is valid.
// This function has real logic with multiple decision points.
func (c *Config) Validate() error {
	if c.MaxRetries < 0 {
		return errors.New("max retries cannot be negative")
	}
	if c.MaxRetries > 10 {
		return errors.New("max retries cannot exceed 10")
	}
	if c.Timeout <= 0 {
		return errors.New("timeout must be positive")
	}
	if c.Timeout > 300 {
		return errors.New("timeout cannot exceed 300 seconds")
	}
	return nil
}

// ProcessItems processes a list of items with actual logic.
// This demonstrates a function with reasonable cyclomatic complexity.
func ProcessItems(items []string, config *Config) ([]string, error) {
	if len(items) == 0 {
		return nil, ErrInvalidInput
	}

	result := make([]string, 0, len(items))
	retries := 0

	for _, item := range items {
		if item == "" {
			continue
		}

		processed := strings.TrimSpace(item)
		if len(processed) == 0 {
			continue
		}

		if strings.HasPrefix(processed, "#") {
			// Skip comments
			continue
		}

		if config.Debug && len(processed) > 100 {
			processed = processed[:100]
		}

		result = append(result, strings.ToLower(processed))

		if len(result) >= MaxConnections {
			break
		}
	}

	if len(result) == 0 && retries < config.MaxRetries {
		return nil, errors.New("no valid items found after processing")
	}

	return result, nil
}

// CalculateScore computes a score based on multiple factors.
func CalculateScore(values []int, threshold int) int {
	if len(values) == 0 {
		return 0
	}

	sum := 0
	count := 0

	for _, v := range values {
		if v < 0 {
			continue
		}
		if v > threshold {
			sum += threshold
		} else {
			sum += v
		}
		count++
	}

	if count == 0 {
		return 0
	}

	avg := sum / count
	if avg > 100 {
		return 100
	}
	return avg
}
