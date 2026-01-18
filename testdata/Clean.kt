// Test fixtures for hollowcheck - clean implementation patterns.
package testdata

/**
 * Represents application configuration.
 */
data class Config(
    val maxRetries: Int = 3,
    val timeout: Int = 30,
    val debug: Boolean = false
)

const val MAX_CONNECTIONS = 100

/**
 * Error for invalid input.
 */
class InvalidInputException(message: String) : Exception(message)

/**
 * Checks if the configuration is valid.
 * This function has real logic with multiple decision points.
 */
fun validateConfig(config: Config) {
    if (config.maxRetries < 0) {
        throw IllegalArgumentException("max retries cannot be negative")
    }
    if (config.maxRetries > 10) {
        throw IllegalArgumentException("max retries cannot exceed 10")
    }
    if (config.timeout <= 0) {
        throw IllegalArgumentException("timeout must be positive")
    }
    if (config.timeout > 300) {
        throw IllegalArgumentException("timeout cannot exceed 300 seconds")
    }
}

/**
 * Processes a list of items with actual logic.
 * This demonstrates a function with reasonable cyclomatic complexity.
 */
fun processItems(items: List<String>, config: Config): List<String> {
    if (items.isEmpty()) {
        throw InvalidInputException("invalid input")
    }

    val result = mutableListOf<String>()
    val retries = 0

    for (item in items) {
        if (item.isEmpty()) {
            continue
        }

        var processed = item.trim()
        if (processed.isEmpty()) {
            continue
        }

        if (processed.startsWith("#")) {
            // Skip comments
            continue
        }

        if (config.debug && processed.length > 100) {
            processed = processed.substring(0, 100)
        }

        result.add(processed.lowercase())

        if (result.size >= MAX_CONNECTIONS) {
            break
        }
    }

    if (result.isEmpty() && retries < config.maxRetries) {
        throw IllegalArgumentException("no valid items found after processing")
    }

    return result
}

/**
 * Computes a score based on multiple factors.
 */
fun calculateScore(values: IntArray, threshold: Int): Int {
    if (values.isEmpty()) {
        return 0
    }

    var sum = 0
    var count = 0

    for (v in values) {
        if (v < 0) {
            continue
        }
        if (v > threshold) {
            sum += threshold
        } else {
            sum += v
        }
        count++
    }

    if (count == 0) {
        return 0
    }

    val avg = sum / count
    if (avg > 100) {
        return 100
    }
    return avg
}
