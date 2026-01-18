// Test fixtures for hollowcheck - clean implementation patterns.
package testdata;

import java.util.ArrayList;
import java.util.List;

/**
 * Represents application configuration.
 */
class Config {
    int maxRetries;
    int timeout;
    boolean debug;

    Config(int maxRetries, int timeout, boolean debug) {
        this.maxRetries = maxRetries;
        this.timeout = timeout;
        this.debug = debug;
    }
}

/**
 * Error for invalid input.
 */
class InvalidInputException extends Exception {
    InvalidInputException(String message) {
        super(message);
    }
}

public class Clean {
    public static final int MAX_CONNECTIONS = 100;

    /**
     * Checks if the configuration is valid.
     * This function has real logic with multiple decision points.
     */
    public static void validateConfig(Config config) throws IllegalArgumentException {
        if (config.maxRetries < 0) {
            throw new IllegalArgumentException("max retries cannot be negative");
        }
        if (config.maxRetries > 10) {
            throw new IllegalArgumentException("max retries cannot exceed 10");
        }
        if (config.timeout <= 0) {
            throw new IllegalArgumentException("timeout must be positive");
        }
        if (config.timeout > 300) {
            throw new IllegalArgumentException("timeout cannot exceed 300 seconds");
        }
    }

    /**
     * Processes a list of items with actual logic.
     * This demonstrates a function with reasonable cyclomatic complexity.
     */
    public static List<String> processItems(List<String> items, Config config) throws InvalidInputException {
        if (items.isEmpty()) {
            throw new InvalidInputException("invalid input");
        }

        List<String> result = new ArrayList<>();
        int retries = 0;

        for (String item : items) {
            if (item == null || item.isEmpty()) {
                continue;
            }

            String processed = item.trim();
            if (processed.isEmpty()) {
                continue;
            }

            if (processed.startsWith("#")) {
                // Skip comments
                continue;
            }

            if (config.debug && processed.length() > 100) {
                processed = processed.substring(0, 100);
            }

            result.add(processed.toLowerCase());

            if (result.size() >= MAX_CONNECTIONS) {
                break;
            }
        }

        if (result.isEmpty() && retries < config.maxRetries) {
            throw new IllegalArgumentException("no valid items found after processing");
        }

        return result;
    }

    /**
     * Computes a score based on multiple factors.
     */
    public static int calculateScore(int[] values, int threshold) {
        if (values.length == 0) {
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
}
