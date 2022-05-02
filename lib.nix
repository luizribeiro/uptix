{ lockFile }:

let
  importJSON = path: builtins.fromJSON (builtins.readFile path);
in
{
  image = name: (importJSON lockFile).${name};
}
