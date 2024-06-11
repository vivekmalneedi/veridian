//------------------------------------------------------------------------------
// BasicClient.cpp
// Diagnostic client that formats to a text string
//
// File is under the MIT license; see LICENSE for details
//------------------------------------------------------------------------------
#include "BasicClient.h"

#include "FormatBuffer.h"

#include "slang/text/SourceManager.h"

namespace slang {

static constexpr auto noteColor = fmt::terminal_color::bright_black;
static constexpr auto warningColor = fmt::terminal_color::bright_yellow;
static constexpr auto errorColor = fmt::terminal_color::bright_red;
static constexpr auto fatalColor = fmt::terminal_color::bright_red;
static constexpr auto highlightColor = fmt::terminal_color::bright_green;
static constexpr auto filenameColor = fmt::terminal_color::cyan;
static constexpr auto locationColor = fmt::terminal_color::bright_cyan;

static fmt::terminal_color getSeverityColor(DiagnosticSeverity severity) {
    switch (severity) {
    case DiagnosticSeverity::Note: return noteColor;
    case DiagnosticSeverity::Warning: return warningColor;
    case DiagnosticSeverity::Error: return errorColor;
    case DiagnosticSeverity::Fatal: return fatalColor;
    default: return fmt::terminal_color::black;
    }
}

BasicClient::SymbolPathCB BasicClient::defaultSymbolPathCB;

BasicClient::BasicClient()
    : buffer(std::make_unique<FormatBuffer>()),
      symbolPathCB(defaultSymbolPathCB) {}

BasicClient::~BasicClient() = default;

void BasicClient::setColorsEnabled(bool enabled) {
    buffer->setColorsEnabled(enabled);
}

void BasicClient::report(const ReportedDiagnostic& diag) {
    if (diag.shouldShowIncludeStack) {
        SmallVector<SourceLocation, 8> includeStack;
        getIncludeStack(diag.location.buffer(), includeStack);

        // Show the stack in reverse.
        for (int i = int(includeStack.size()) - 1; i >= 0; i--) {
            SourceLocation loc = includeStack[size_t(i)];
            buffer->format("in file included from {}:{}:\n",
                           sourceManager->getFileName(loc),
                           sourceManager->getLineNumber(loc));
        }
    }

    // Print out the hierarchy where the diagnostic occurred, if we know it.
    auto& od = diag.originalDiagnostic;
    if (od.coalesceCount && od.symbol && symbolPathCB) {
        if (od.coalesceCount == 1)
            buffer->append("  in instance: "sv);
        else
            buffer->format("  in {} instances, e.g. ", *od.coalesceCount);

        buffer->append(fmt::emphasis::bold, symbolPathCB(*od.symbol));
        buffer->append("\n"sv);
    }

    // Get all highlight ranges mapped into the reported location of the
    // diagnostic.
    SmallVector<SourceRange, 8> mappedRanges;
    engine->mapSourceRanges(diag.location, diag.ranges, mappedRanges);

    // Write the diagnostic.
    formatDiag(diag.location, diag.severity, diag.formattedMessage,
               engine->getOptionName(diag.originalDiagnostic.code));

    // Write out macro expansions, if we have any, in reverse order.
    for (auto it = diag.expansionLocs.rbegin(); it != diag.expansionLocs.rend();
         it++) {
        SourceLocation loc = *it;
        std::string name(sourceManager->getMacroName(loc));
        if (name.empty())
            name = "expanded from here";
        else
            name = fmt::format("expanded from macro '{}'", name);

        SmallVector<SourceRange, 8> macroRanges;
        engine->mapSourceRanges(loc, diag.ranges, macroRanges);
        formatDiag(sourceManager->getFullyOriginalLoc(loc),
                   DiagnosticSeverity::Note, name, "");
    }
}

void BasicClient::clear() {
    buffer->clear();
}

std::string BasicClient::getString() const {
    return buffer->str();
}

void BasicClient::formatDiag(SourceLocation loc, DiagnosticSeverity severity,
                             std::string_view message, std::string_view optionName) {
    size_t col = 0;
    if (loc != SourceLocation::NoLocation) {
        col = sourceManager->getColumnNumber(loc);
        buffer->append(fg(filenameColor), sourceManager->getFileName(loc));
        buffer->append(":");
        buffer->format(fg(locationColor), "{}:{}",
                       sourceManager->getLineNumber(loc), col);
        buffer->append(": ");
    }

    buffer->format(fg(getSeverityColor(severity)),
                   "{}: ", getSeverityString(severity));

    if (severity != DiagnosticSeverity::Note)
        buffer->format(fmt::text_style(fmt::emphasis::bold), "{}", message);
    else
        buffer->append(message);

    if (!optionName.empty())
        buffer->format(" [-W{}]", optionName);

    buffer->append("\n"sv);
}

} // namespace slang
