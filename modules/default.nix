{ lockFile }:

let
  importJSON = path: builtins.fromJSON (builtins.readFile path);
  lockFor = key: (importJSON lockFile).${key};
in
{
  dockerImage = name: "${name}@${lockFor name}";
}
