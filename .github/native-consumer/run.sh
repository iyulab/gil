#!/bin/sh
# Runs the consumer in this folder against the packages in ./packages (run from the repository root): a console app
# that takes Gil, Gil.Review and Gil.Tree from that folder only — the same version may also be on NuGet.org — and
# everything else from NuGet.org. POSIX sh, so that it runs in the Alpine .NET image as well as on the runners.
set -eu

# NuGet on Windows reads a Windows path: Git Bash gives one with pwd -W, other shells refuse the flag.
root=$(pwd -W 2>/dev/null || pwd)
gil=$(ls packages | sed -n 's/^Gil\.\([0-9].*\)\.nupkg$/\1/p')
review=$(ls packages | sed -n 's/^Gil\.Review\.\(.*\)\.nupkg$/\1/p')
work=$(mktemp -d)
dotnet new console -o "$work/consumer" --framework net10.0
cd "$work/consumer"
cat > nuget.config <<EOF
<configuration>
  <packageSources>
    <clear />
    <add key="built" value="$root/packages" />
    <add key="nuget" value="https://api.nuget.org/v3/index.json" />
  </packageSources>
  <packageSourceMapping>
    <packageSource key="built">
      <package pattern="Gil" />
      <package pattern="Gil.*" />
    </packageSource>
    <packageSource key="nuget">
      <package pattern="*" />
    </packageSource>
  </packageSourceMapping>
</configuration>
EOF
dotnet add package Gil --version "$gil"
dotnet add package Gil.Review --version "$review"
dotnet add package Gil.Tree --version "$gil"
cp "$root/.github/native-consumer/Program.cs" Program.cs
dotnet run -c Release
