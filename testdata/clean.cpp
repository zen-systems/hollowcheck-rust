// Test fixtures for hollowcheck - clean implementation patterns.

#include <string>
#include <vector>
#include <stdexcept>
#include <algorithm>
#include <cctype>

namespace testdata {

/**
 * Represents application configuration.
 */
struct Config {
    int maxRetries = 3;
    int timeout = 30;
    bool debug = false;
};

// Maximum number of allowed connections.
constexpr int MAX_CONNECTIONS = 100;

/**
 * Error for invalid input.
 */
class InvalidInputError : public std::exception {
public:
    const char* what() const noexcept override {
        return "invalid input";
    }
};

/**
 * Helper to trim whitespace from a string.
 */
static std::string trim(const std::string& str) {
    auto start = str.find_first_not_of(" \t\n\r");
    if (start == std::string::npos) return "";
    auto end = str.find_last_not_of(" \t\n\r");
    return str.substr(start, end - start + 1);
}

/**
 * Helper to convert string to lowercase.
 */
static std::string toLower(const std::string& str) {
    std::string result = str;
    std::transform(result.begin(), result.end(), result.begin(),
                   [](unsigned char c) { return std::tolower(c); });
    return result;
}

/**
 * Checks if the configuration is valid.
 * This function has real logic with multiple decision points.
 */
void validateConfig(const Config& config) {
    if (config.maxRetries < 0) {
        throw std::invalid_argument("max retries cannot be negative");
    }
    if (config.maxRetries > 10) {
        throw std::invalid_argument("max retries cannot exceed 10");
    }
    if (config.timeout <= 0) {
        throw std::invalid_argument("timeout must be positive");
    }
    if (config.timeout > 300) {
        throw std::invalid_argument("timeout cannot exceed 300 seconds");
    }
}

/**
 * Processes a list of items with actual logic.
 * This demonstrates a function with reasonable cyclomatic complexity.
 */
std::vector<std::string> processItems(const std::vector<std::string>& items, const Config& config) {
    if (items.empty()) {
        throw InvalidInputError();
    }

    std::vector<std::string> result;
    result.reserve(items.size());
    int retries = 0;

    for (const auto& item : items) {
        if (item.empty()) {
            continue;
        }

        std::string processed = trim(item);
        if (processed.empty()) {
            continue;
        }

        if (processed[0] == '#') {
            // Skip comments
            continue;
        }

        if (config.debug && processed.length() > 100) {
            processed = processed.substr(0, 100);
        }

        result.push_back(toLower(processed));

        if (static_cast<int>(result.size()) >= MAX_CONNECTIONS) {
            break;
        }
    }

    if (result.empty() && retries < config.maxRetries) {
        throw std::runtime_error("no valid items found after processing");
    }

    return result;
}

/**
 * Computes a score based on multiple factors.
 */
int calculateScore(const std::vector<int>& values, int threshold) {
    if (values.empty()) {
        return 0;
    }

    int sum = 0;
    int count = 0;

    for (int v : values) {
        if (v < 0) {
            continue;
        }
        if (v > threshold) {
            sum += threshold;
        } else {
            sum += v;
        }
        count++;
    }

    if (count == 0) {
        return 0;
    }

    int avg = sum / count;
    if (avg > 100) {
        return 100;
    }
    return avg;
}

} // namespace testdata
