{
  description = "release train language-family-slice-three (42df158c9708d7f06c980a9431a51c4952d92560d9bfa9ce27de45b9288e6cea)";
  inputs = {
    content_identity = { url = "github:LiGoldragon/content-identity/3f705566f36d171d9fa98167ba2b71f6e9a9f93d"; narHash = "sha256-5BaO8NGgkaEDKdV8tJ/62e9WJNYN8KwpxHlB/AilTSY="; };
    core_schema = { url = "github:LiGoldragon/core-schema/361c19fb43d87ec4945b726f64fe7bd932a0fcc6"; narHash = "sha256-+TrUWxBfLCdQWoThfyqE7N8DUruqSjk2eCP3kw43NNw="; };
    name_table = { url = "github:LiGoldragon/name-table/1c1d6ff6f5824402dcef3b1005b14465b4e90cdb"; narHash = "sha256-iVjecRxiy3/Ol0Ct+hshELRWXwwiaU6zR4OTQL29IHk="; };
    raw_discovery = { url = "github:LiGoldragon/raw-discovery/b6cc1c8d80a8b4812ddf29317d3f50e04d5fc838"; narHash = "sha256-7gtw0HSLYbir4VANNda7zL3HrQ3vILbUFguFStu3Chk="; };
    structural_codec = { url = "github:LiGoldragon/structural-codec/3a1d56770502ffe7f3745187c118fc79db1a4f9a"; narHash = "sha256-OkRUkvHk7GqH+LFD+YnJFCcRChj6KiSxL4hkSFBwvUo="; };
    structural_codec_derive = { url = "github:LiGoldragon/structural-codec-derive/e77619494e7dd4c14d570ed002c83a6d88b4b9f0"; narHash = "sha256-irc3WbPmTHUfun0J6z4wAvGiJLbiKuQCl6Iy06eH3V0="; };
  };
  outputs = inputs: {
    releaseTrain = builtins.fromJSON (builtins.readFile ./release-train.lock.json);
  };
}
