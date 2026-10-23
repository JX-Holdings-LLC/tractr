using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace FarmEngine.Interop;

/// <summary>
/// Serializer settings for the JSON the Interop layer exchanges with farm-ffi besides the schema
/// records (which go through <c>RecordJson</c>): requests, commands, frame info and answers. Property names are
/// camelCase, as the engine's JSON is; <c>null</c> means "absent" and is omitted, except for
/// properties annotated <c>[JsonIgnore(Condition = Never)]</c>, which are present-as-null.
/// </summary>
public static class InteropJson
{
    public static readonly JsonSerializerOptions Options = Create(indented: false);
    public static readonly JsonSerializerOptions Indented = Create(indented: true);

    private static JsonSerializerOptions Create(bool indented)
    {
        var options = new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
            DictionaryKeyPolicy = null,
            DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
            AllowOutOfOrderMetadataProperties = true,
            PropertyNameCaseInsensitive = false,
            ReadCommentHandling = JsonCommentHandling.Skip,
            AllowTrailingCommas = true,
            // JSON has no NaN/Infinity, and the engine refuses them.
            NumberHandling = JsonNumberHandling.Strict,
            WriteIndented = indented,
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
            RespectNullableAnnotations = false,
        };
        options.MakeReadOnly(populateMissingResolver: true);
        return options;
    }

    public static T? Deserialize<T>(string json) => JsonSerializer.Deserialize<T>(json, Options);

    public static string Serialize<T>(T value, bool indented = false) =>
        JsonSerializer.Serialize(value, indented ? Indented : Options);

    public static JsonElement ToElement<T>(T value) => JsonSerializer.SerializeToElement(value, Options);

    /// <summary><c>JSON.parse(JSON.stringify(value))</c> — a structural deep copy.</summary>
    public static T DeepClone<T>(T value) =>
        JsonSerializer.Deserialize<T>(JsonSerializer.SerializeToUtf8Bytes(value, Options), Options)!;
}
