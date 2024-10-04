import { S3 } from 'aws-sdk';
import { DocumentClient } from 'aws-sdk/clients/dynamodb';
export const s3 = new S3();
export const dynamodb = new DocumentClient();