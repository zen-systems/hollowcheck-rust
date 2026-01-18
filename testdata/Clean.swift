// Swift test fixture for hollowcheck - clean implementation
import Foundation

/// Config represents application configuration.
struct Config {
    var maxRetries: Int
    var timeout: Int
    var debug: Bool
}

/// MaxConnections is the maximum number of allowed connections.
let MaxConnections = 100

/// InvalidInputError is returned when input validation fails.
enum ValidationError: Error {
    case invalidInput(String)
}

/// Validate checks if the configuration is valid.
/// This function has real logic with multiple decision points.
func validate(config: Config) throws {
    if config.maxRetries < 0 {
        throw ValidationError.invalidInput("max retries cannot be negative")
    }
    if config.maxRetries > 10 {
        throw ValidationError.invalidInput("max retries cannot exceed 10")
    }
    if config.timeout <= 0 {
        throw ValidationError.invalidInput("timeout must be positive")
    }
    if config.timeout > 300 {
        throw ValidationError.invalidInput("timeout cannot exceed 300 seconds")
    }
}

/// ProcessItems processes a list of items with actual logic.
/// This demonstrates a function with reasonable cyclomatic complexity.
func processItems(items: [String], config: Config) throws -> [String] {
    if items.isEmpty {
        throw ValidationError.invalidInput("invalid input")
    }

    var result: [String] = []

    for item in items {
        if item.isEmpty {
            continue
        }

        let processed = item.trimmingCharacters(in: .whitespaces)
        if processed.isEmpty {
            continue
        }

        if processed.hasPrefix("#") {
            // Skip comments
            continue
        }

        var finalItem = processed
        if config.debug && processed.count > 100 {
            finalItem = String(processed.prefix(100))
        }

        result.append(finalItem.lowercased())

        if result.count >= MaxConnections {
            break
        }
    }

    if result.isEmpty {
        throw ValidationError.invalidInput("no valid items found after processing")
    }

    return result
}

/// CalculateScore computes a score based on multiple factors.
func calculateScore(values: [Int], threshold: Int) -> Int {
    if values.isEmpty {
        return 0
    }

    var sum = 0
    var count = 0

    for v in values {
        if v < 0 {
            continue
        }
        if v > threshold {
            sum += threshold
        } else {
            sum += v
        }
        count += 1
    }

    if count == 0 {
        return 0
    }

    let avg = sum / count
    if avg > 100 {
        return 100
    }
    return avg
}
