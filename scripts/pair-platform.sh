# Sourced by the pair scripts: the target this machine builds. Windows builds
# run in Git Bash inside the MSVC developer environment.
case "$(uname -s)" in
    MINGW* | MSYS*)
        pair_target=win-x64 pair_exe=.exe
        # jq writes CRLF on Windows unless asked for binary output.
        jq() { command jq -b "$@"; }
        ;;
    *) pair_target=linux-x64 pair_exe= ;;
esac
