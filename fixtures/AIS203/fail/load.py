from transformers import AutoModel

model = AutoModel.from_pretrained("org/name", trust_remote_code=True)
