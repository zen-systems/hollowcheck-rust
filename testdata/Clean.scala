// Scala test fixture for hollowcheck - clean implementation
package testdata

/** Config represents application configuration. */
case class Config(maxRetries: Int, timeout: Int, debug: Boolean)

object Clean {
  /** MaxConnections is the maximum number of allowed connections. */
  val MaxConnections = 100

  /** ErrInvalidInput is returned when input validation fails. */
  case class InvalidInputError(message: String) extends Exception(message)

  /** Validate checks if the configuration is valid.
    * This function has real logic with multiple decision points.
    */
  def validate(config: Config): Either[String, Unit] = {
    if (config.maxRetries < 0) {
      Left("max retries cannot be negative")
    } else if (config.maxRetries > 10) {
      Left("max retries cannot exceed 10")
    } else if (config.timeout <= 0) {
      Left("timeout must be positive")
    } else if (config.timeout > 300) {
      Left("timeout cannot exceed 300 seconds")
    } else {
      Right(())
    }
  }

  /** ProcessItems processes a list of items with actual logic.
    * This demonstrates a function with reasonable cyclomatic complexity.
    */
  def processItems(items: Seq[String], config: Config): Either[String, Seq[String]] = {
    if (items.isEmpty) {
      return Left("invalid input")
    }

    var result = Seq.empty[String]

    for (item <- items) {
      if (item.isEmpty) {
        // Skip empty items
      } else {
        val processed = item.trim
        if (processed.isEmpty) {
          // Skip whitespace-only items
        } else if (processed.startsWith("#")) {
          // Skip comments
        } else {
          val finalItem = if (config.debug && processed.length > 100) {
            processed.take(100)
          } else {
            processed
          }

          result = result :+ finalItem.toLowerCase

          if (result.length >= MaxConnections) {
            return Right(result)
          }
        }
      }
    }

    if (result.isEmpty) {
      Left("no valid items found after processing")
    } else {
      Right(result)
    }
  }

  /** CalculateScore computes a score based on multiple factors. */
  def calculateScore(values: Seq[Int], threshold: Int): Int = {
    if (values.isEmpty) {
      return 0
    }

    var sum = 0
    var count = 0

    for (v <- values) {
      if (v >= 0) {
        if (v > threshold) {
          sum += threshold
        } else {
          sum += v
        }
        count += 1
      }
    }

    if (count == 0) {
      0
    } else {
      val avg = sum / count
      if (avg > 100) 100 else avg
    }
  }
}
