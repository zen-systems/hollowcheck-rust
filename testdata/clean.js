// Test fixtures for hollowcheck - clean implementation patterns.

const MAX_CONNECTIONS = 100;

/**
 * Represents application configuration.
 */
class Config {
  constructor(maxRetries = 3, timeout = 30, debug = false) {
    this.maxRetries = maxRetries;
    this.timeout = timeout;
    this.debug = debug;
  }
}

/**
 * Error for invalid input.
 */
class InvalidInputError extends Error {
  constructor(message) {
    super(message);
    this.name = "InvalidInputError";
  }
}

/**
 * Checks if the configuration is valid.
 * This function has real logic with multiple decision points.
 */
function validateConfig(config) {
  if (config.maxRetries < 0) {
    throw new Error("max retries cannot be negative");
  }
  if (config.maxRetries > 10) {
    throw new Error("max retries cannot exceed 10");
  }
  if (config.timeout <= 0) {
    throw new Error("timeout must be positive");
  }
  if (config.timeout > 300) {
    throw new Error("timeout cannot exceed 300 seconds");
  }
}

/**
 * Processes a list of items with actual logic.
 * This demonstrates a function with reasonable cyclomatic complexity.
 */
function processItems(items, config) {
  if (items.length === 0) {
    throw new InvalidInputError("invalid input");
  }

  const result = [];
  const retries = 0;

  for (const item of items) {
    if (!item) {
      continue;
    }

    let processed = item.trim();
    if (processed.length === 0) {
      continue;
    }

    if (processed.startsWith("#")) {
      // Skip comments
      continue;
    }

    if (config.debug && processed.length > 100) {
      processed = processed.substring(0, 100);
    }

    result.push(processed.toLowerCase());

    if (result.length >= MAX_CONNECTIONS) {
      break;
    }
  }

  if (result.length === 0 && retries < config.maxRetries) {
    throw new Error("no valid items found after processing");
  }

  return result;
}

/**
 * Computes a score based on multiple factors.
 */
function calculateScore(values, threshold) {
  if (values.length === 0) {
    return 0;
  }

  let sum = 0;
  let count = 0;

  for (const v of values) {
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

  if (count === 0) {
    return 0;
  }

  const avg = Math.floor(sum / count);
  if (avg > 100) {
    return 100;
  }
  return avg;
}

module.exports = { Config, MAX_CONNECTIONS, InvalidInputError, validateConfig, processItems, calculateScore };
