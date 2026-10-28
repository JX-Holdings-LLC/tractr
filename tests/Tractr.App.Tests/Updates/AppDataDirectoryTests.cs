using Tractr.Updates;

namespace Tractr.App.Tests.Updates;

public sealed class AppDataDirectoryTests
{
    [Fact]
    public void MacUsesApplicationSupportEvenWhenDotnetReturnsConfig()
    {
        Assert.Equal(Path.Combine("home", "Library", "Application Support", "Tractr"),
            AppDataDirectory.Resolve(null, true, "home", "config"));
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void PortableOverrideWinsOnEveryPlatform(bool macOS)
    {
        Assert.Equal("portable", AppDataDirectory.Resolve("portable", macOS, "home", "config"));
    }

    [Fact]
    public void OtherPlatformsRespectApplicationData()
    {
        Assert.Equal(Path.Combine("config", "Tractr"), AppDataDirectory.Resolve(" ", false, "home", "config"));
    }
}
