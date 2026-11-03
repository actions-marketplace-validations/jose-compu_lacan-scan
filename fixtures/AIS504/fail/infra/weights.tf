resource "aws_s3_bucket" "checkpoints" {
  bucket = "ml-weights-demo"
  acl    = "public-read"
}
