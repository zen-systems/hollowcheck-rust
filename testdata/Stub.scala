// Scala test fixture for hollowcheck - stub implementations
package testdata

/** StubConfig is a placeholder configuration type. */
case class StubConfig(name: String)

/** DefaultTimeout is a stub constant. */
object Constants {
  val DefaultTimeout = 30
}

object Stub {
  /** ProcessData is a stub function that does nothing useful.
    * TODO: implement actual processing logic
    */
  def processData(input: String): Either[String, Unit] = {
    // FIXME: this is a placeholder implementation
    Right(())
  }

  /** ValidateInput is another stub.
    * HACK: bypassing validation for now
    */
  def validateInput(data: Array[Byte]): Boolean = {
    // XXX: need to add proper validation
    true
  }

  /** HandleRequest is a stub handler. */
  def handleRequest(): Unit = {
    throw new NotImplementedError("not implemented")
  }

  /** NotImplementedFunc demonstrates a stub pattern. */
  def notImplementedFunc(): Unit = {
    // This function intentionally left empty
    ()
  }
}
