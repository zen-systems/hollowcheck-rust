// Scala test fixture for hollowcheck - mock data
package testdata

/** MockUser represents a user with fake data. */
case class MockUser(id: String, email: String, name: String)

/** MockConfig has placeholder values. */
case class MockConfig(apiKey: String, endpoint: String, password: String)

object Mock {
  /** GetTestUser returns a user with mock data. */
  def getTestUser(): MockUser = {
    MockUser(
      id = "12345",
      email = "test@example.com",
      name = "foo"
    )
  }

  /** GetAnotherUser returns another mock user. */
  def getAnotherUser(): MockUser = {
    MockUser(
      id = "00000",
      email = "user@example.com",
      name = "bar"
    )
  }

  /** GetMockConfig returns a config with fake data. */
  def getMockConfig(): MockConfig = {
    MockConfig(
      apiKey = "asdf1234",
      endpoint = "https://api.example.com/v1",
      password = "changeme"
    )
  }

  /** GetDescription returns lorem ipsum placeholder text. */
  def getDescription(): String = {
    "lorem ipsum dolor sit amet"
  }

  /** GetPhoneNumber returns a placeholder phone. */
  def getPhoneNumber(): String = {
    "xxx-xxx-xxxx"
  }

  /** SequentialIDs returns fake sequential IDs. */
  def sequentialIDs(): Seq[String] = {
    Seq("11111", "22222", "33333")
  }
}
